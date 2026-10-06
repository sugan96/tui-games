# Brick ball implementation plan

Tasks 5 to 10 record the play test iterations as a design log; their code landed folded into fewer commits, so the task to commit mapping differs from this file for those tasks.

Spec: docs/spec.md

## Global constraints

- Commit messages: exactly one line, conventional commits (feat(scope): ..., docs: ...). No body, no trailers of any kind.
- One commit per task. The build must pass at every commit. Review fix rounds amend the task's commit (git commit --amend, or fixup + autosquash) so the branch never carries "fix review" commits.
- cargo build, cargo test and cargo clippy must pass with no warnings.
- No em dashes anywhere in code, comments or docs. Use a plain dash.
- No new dependencies beyond those already in the workspace Cargo.toml.
- Exact values come from docs/spec.md, section "Brick ball" and the stage progress paragraphs in "Game menu, name entry, game over (SDK)" and "Storage". The plan repeats them; it never changes them.
- The rules module of brick ball imports nothing from ratatui or rusqlite.
- Tests are plain #[cfg(test)] modules, no test frameworks. Every task leaves tests that fail if its logic breaks.
- Implementers do not dispatch subagents.
- Snake keeps its rules and shares the SDK's held key rule: its tests pass with only the signature edits the SDK tasks require.

Workspace dependencies available: anyhow 1, rand 0.10, ratatui 0.30, rusqlite 0.40 bundled, arcade-sdk by path. rand 0.10 API used here: `use rand::{RngExt, SeedableRng, rngs::StdRng};`, `StdRng::seed_from_u64(s)`, `rng.random_range(0..n)`, `rng.random::<f64>()`, `rng.random_bool(p)`, and `rand::random::<u64>()` for a run seed.

Values from the spec that this plan uses, repeated here so every task reads the same numbers:

- Board: 60 by 44 pixels in 60 by 22 cells, `x` 0 to 59, `y` 0 to 43, 0 at the top. Brick grid 15 columns of 4 by 1 pixel bricks, 10 rows at `y` 4 to 13. Paddle 1 pixel tall at `y` 42. Ball 1 pixel. HUD panel 16 columns, border included. The play screen fits 80 by 24, so `min_size` is (80, 24); the program needs 80 by 28 because the SDK game menu is 28 rows tall.
- Tick 16.67 ms while a ball is in play, 33 ms on serve, stage clear and pause, 200 ms per death blink half-period. Timers count real seconds, not ticks. Paddle speed rises evenly from 30 to 90 px/s over the first 0.2 s of a hold. While a hold is provisional the paddle moves at 30 px/s, and the ramp starts when a repeat confirms the hold. Brick ball sets `hold_on_press`.
- Sawtooth: blocks of 5, `e = n - 2` when `n > 1` and `n % 5 == 1`, else `e = n`.
- `g(e, k) = 1 - exp(-e / k)`. Ball speed `28 × (1 + 0.7 × g(e, 20))` px/s. Brick count `round(150 × (0.45 + 0.40 × g(e, 15)))`. Brick HP `1 + floor(4 × g(e, 25) + r)`, `r` uniform in [0, 1), never above 5. Paddle width `9 - round(3 × g(e, 30))`. Unbreakable bricks none before stage 8, then `round(count × 0.10 × g(e - 8, 20))`.
- Lever table, floor (e = 0) to cap: speed 28 to 47.6, count 68 to 128, HP all 1 to all 5, paddle 9 to 6, unbreakable 0 to about 10%.
- Stage progress: the picker value `n` is 1 when the game opens and keeps its value between runs. `Game::reached` has a default of 0. The SDK raises the saved progress to `game.reached()` when a run ends and when the player quits with `q` during a run. Progress is written when `reached` is greater than the saved value, where no row counts as 0.
- Lives 3, plus 1 on clearing a stage whose number is a multiple of 5, never more than 5.
- Score `10 + n` per hit on a breakable brick, `100 × n` per clear.
- Paddle angle `60° × t` from straight up, `t` from -1 at the left edge to 1 at the right edge.
- Capsule chance 8% per broken brick, only when no capsule is falling. Falls at 20 px/s. `W` +3 pixels for 15 s, `S` 0.75 speed for 10 s, `M` every ball splits into 3, up to 9 balls.
- Death blink: six half-periods of 200 ms.
- Stage clear: countdown of 2 seconds.
- Colors (xterm 256): field 233. HP 1 to 5 light/dark: 223/180, 215/173, 209/167, 167/131, 124/88. Unbreakable 245/239. Paddle 173. Ball 255. Capsule a 1 by 2 pixel block in its power color, `W` 45, `S` 82, `M` 201. HUD labels amber 214, score terracotta.
- Variant `stage <start>-<reached>`. Summary `▬ <bricks> bricks`.

## Task 1: SDK stage progress

Add stage progress to the SDK: the `Entry::stages` flag, the stage argument to `start`, the default method `Game::reached`, the `progress` table, the stage picker on the game menu, and the raise when a run ends or the player quits with `q` during a run. `Outcome` does not change. Snake gets only the edits the new `Entry` field and the new `start` signature force; it does not implement `reached`.

Files changed:

- `crates/sdk/src/game.rs`
- `crates/sdk/src/db.rs`
- `crates/sdk/src/app.rs`
- `crates/sdk/src/ui/menu.rs`
- `crates/sdk/src/ui/mod.rs` (tests only: the `use crate::app::tests::{app, start}` line gains `staged_app`; the `outcome()` fixture is unchanged because `Outcome` is unchanged)
- `crates/sdk/src/ui/theme.rs`
- `crates/sdk/src/testkit.rs`
- `games/snake/src/main.rs`
- `docs/adding-a-game.md`

Public interface, exact:

```rust
// game.rs
pub struct Entry {
    pub id: &'static str,
    pub title: &'static str,
    pub about: &'static str,
    pub starts: &'static [(&'static str, &'static str)],
    /// True: the game menu shows a stage picker and `start` gets the chosen
    /// stage. False: no picker, `start` gets 1 and `Game::reached` is never read.
    pub stages: bool,
    /// Starts a game for a key pressed on the game menu and a stage. Enter
    /// arrives as '\n'. None for keys the game does not use. q is never passed.
    pub start: fn(char, u32) -> Option<Box<dyn Game>>,
    pub min_size: (u16, u16),
    pub thumb: &'static [&'static str],
}

pub trait Game {
    // ... existing methods: key, tick, tick_rate, draw ...
    /// Highest stage this run has got to. The SDK raises the saved progress to
    /// it when the run ends and when the player quits with q during the run.
    /// The default 0 is for games without stages, which leave it out.
    fn reached(&self) -> u32 {
        0
    }
}

// Outcome is unchanged: score, variant, summary.

// db.rs
impl Db {
    /// Highest stage reached in `game`, 1 when no row exists.
    pub fn reached(&self, game: &str) -> Result<u32>;
    /// Raises the saved stage of `game` to `reached` when `reached` is greater
    /// than the saved value, where no row counts as 0. Never lowers it.
    pub fn raise(&self, game: &str, reached: u32) -> Result<()>;
    /// Rows in the progress table. For tests in other modules, which cannot
    /// read the private `conn`.
    #[cfg(test)]
    pub fn progress_rows(&self) -> u32;
}

// app.rs, new fields on App
pub struct App {
    // ... existing fields ...
    /// Stage chosen on the menu picker, 1 to max_stage.
    pub stage: u32,
    /// Highest stage reached in this game, at least 1. Always 1 for a game without stages.
    pub max_stage: u32,
}

// theme.rs
/// The `STAGE n` chip of the menu stage picker.
pub const PICKER: Style = Style::new().fg(c(233)).bg(c(214)).add_modifier(Modifier::BOLD);
```

Schema, in `db.rs`:

```rust
const PROGRESS: &str = "CREATE TABLE IF NOT EXISTS progress (
  game TEXT PRIMARY KEY,
  reached INTEGER NOT NULL
);";
```

Behaviour:

- `Db::init` runs `PROGRESS` on every open, after the existing `user_version` block and inside the same transaction, so a database already at `user_version` 1 gains the table without a migration. `user_version` stays 1.
- `Db::reached` reads `SELECT reached FROM progress WHERE game = ?1` with `rusqlite::OptionalExtension::optional`, and returns `max(1, value)` or 1 when there is no row.
- `Db::raise` implements the spec's guard: progress is written when `reached > saved`, where no row counts as 0. It returns `Ok(())` at once when `reached == 0`, because 0 is never greater than the saved value. Otherwise it runs `INSERT INTO progress (game, reached) VALUES (?1, ?2) ON CONFLICT (game) DO UPDATE SET reached = excluded.reached WHERE excluded.reached > progress.reached`. With no row, any `reached >= 1` is greater than 0 and inserts, so the first run writes a row even at stage 1. With a row, the `WHERE` writes only when the run reached further. The guard compares against the stored value or 0, not against `Db::reached`, which reports 1 for no row.
- `Db::progress_rows` runs `SELECT COUNT(*) FROM progress`. It is `#[cfg(test)]` and exists because `conn` is private to `db.rs`.
- `App::new` sets `stage = 1` and `max_stage = db.reached(entry.id)?` when `entry.stages`, else `max_stage = 1`.
- `App::handle_key` on `Screen::Menu`:
  - `KeyCode::Left` when `entry.stages`: `stage = max(1, stage - 1)`.
  - `KeyCode::Right` when `entry.stages`: `stage = min(max_stage, stage + 1)`.
  - `KeyCode::Char(c)` calls `(entry.start)(c, s)` and `KeyCode::Enter` calls `(entry.start)('\n', s)`, where `s` is `stage` when `entry.stages` and 1 otherwise. A `Some` game starts play as today.
  - Every other key does nothing. For a game without stages the arrows do nothing, as today.
- From the spec: `n` is 1 when the game opens and keeps its value between runs. So `stage` is set to 1 only in `App::new`. Coming back to the menu with `r` keeps the last choice, which is always within 1 to `max_stage` because `max_stage` only rises.
- A private `fn save_progress(&mut self, reached: u32) -> anyhow::Result<()>` on `App` calls `db.raise(entry.id, reached)?` and sets `max_stage = max(max_stage, reached)`. It is called only when `entry.stages`, so for a game without stages `reached` is never read.
- `App::on_timer`, when a game returns `Over`: after `db.record`, when `entry.stages`, call `save_progress(game.reached())`.
- `App::handle_key`, on `q` while the screen is `Screen::Playing(game)` and `entry.stages`: read `game.reached()`, call `save_progress` with it, then set `quit` as today. So a long run quit early keeps its unlocks. On the other screens `q` saves nothing: after `Over` the progress is already saved.
- `ui/menu.rs::draw`, when `entry.stages`: on row `y + 21`, the line directly above the start prompt at `y + 22`, draw centered with `center_line` the spans `"◀ "`, `"STAGE {stage}"`, `" ▶"`, `" of {max_stage}"`. The text reads `◀ STAGE 3 ▶ of 7`, as the spec writes it. Styles: an arrow that can move is `theme::AMBER`; the left arrow when `stage == 1` and the right arrow when `stage == max_stage` are `theme::CHROME_DIM`; the stage chip is `theme::PICKER`; `of m` is `theme::MUTED`. The picker is not drawn when `entry.stages` is false, so snake's menu is unchanged.
- Plan decision, from the mockup: when `entry.stages` the bottom border hints are `("←→", "stage")`, then `entry.starts`, then `("q", "quit")`. Without stages the hints are unchanged.
- `testkit::check`:
  - Start keys are `(' '..='~').chain(['\n'])`, minus `q`, for which `(e.start)(c, 1)` is `Some`.
  - Each key is run once per stage in `[1]`, or in `[1, CHECK_STAGE]` when `e.stages`, with `const CHECK_STAGE: u32 = 20;` (plan decision: stage 20 has unbreakable bricks and a raised speed). Each run starts with `(e.start)(c, stage)`.
  - When `e.stages` and the run ends, assert `g.reached() >= stage`, because the start stage counts as reached. This also catches a stage game that forgot to implement `reached` and returns the default 0.
  - Panic messages print the key with `{c:?}` so `'\n'` reads clearly.
- Snake, `games/snake/src/main.rs`: `ENTRY` gains `stages: false`. `fn start(c: char, _stage: u32) -> Option<Box<dyn Game>>` ignores the stage. Its `Outcome` and its `Game` impl do not change; it keeps the default `reached`. In the tests, `start('5')` becomes `start('5', 1)` and the same for the other three calls. Nothing else changes.
- `docs/adding-a-game.md`:
  - The `Entry` example gains `stages: false` with a comment that true shows a stage picker on the menu and passes the chosen stage.
  - The `start` comment becomes `fn(char, u32) -> Option<Box<dyn Game>>`.
  - The sentence about `start` says it gets every key pressed on the game menu except `q`, Enter as `'\n'`, and the chosen stage, or 1 without stages.
  - A new bullet after the `Outcome` bullet says a game with stages implements `fn reached(&self) -> u32`, the highest stage the run has got to, which the SDK saves as progress when the run ends and when the player quits with `q` during a run. A game without stages leaves it out and gets the default 0. The `Outcome` bullet does not change.
  - The `check` paragraph says it also starts a game with stages at stage 20 and checks `reached`.

Test fixtures in `app.rs` tests:

- `FAKE` gains `stages: false`.
- `Fake` gains `reached: u32` and implements `fn reached(&self) -> u32 { self.reached }`. `fake_start(c: char, stage: u32)` keeps the digit score and sets `reached = stage + 1`, so a test fails if the SDK saves progress for a game without stages.
- Add `staged_start(c: char, stage: u32)`, which starts only on `'\n'`, with score 0 (so the run ends on game over, not name entry) and `reached = stage + 2`, so the run has got further than its start from the first tick.
- Add `pub static STAGED: Entry`, a full struct literal with id `"staged"`, title `"staged"`, `stages: true`, `starts: &[("enter", "start")]`, `start: staged_start`, and the same `min_size` and `thumb` as `FAKE`.
- Add `pub fn staged_app(max: u32) -> App`, which opens an in-memory db, calls `raise("staged", max)` and builds the App. The `start(c)` helper keeps one argument and calls `fake_start(c, 1)`.

Tests:

- `db.rs`
  - `progress_defaults_to_one_and_round_trips`: `reached("g")` is 1 and `progress_rows()` is 0. After `raise("g", 4)` it is 4.
  - `first_raise_writes_a_row_even_at_stage_one`: on a fresh db, `raise("g", 0)` leaves `progress_rows()` at 0. `raise("g", 1)` makes it 1 and `reached("g")` is 1.
  - `progress_only_rises`: after `raise 4` then `raise 2` it is 4. After `raise 7` it is 7. `reached("other")` is still 1.
  - `existing_database_gains_progress_table`: a connection with `SCHEMA` run, `PRAGMA user_version = 1` and no `progress` table goes through `Db::init`. Then `raise` and `reached` work and `user_version` is still 1.
- `app.rs`
  - `stage_picker_moves_within_one_to_max`: `staged_app(3)`. Left at 1 stays 1. Right three times gives 3. Left gives 2.
  - `enter_starts_the_chosen_stage_and_raises_progress`: `staged_app(3)`. Right once, then Enter: the app is playing. After `finish`, `db.reached("staged")` is 4 and `max_stage` is 4.
  - `picker_keeps_its_value_between_runs`: `staged_app(3)`. Right once, Enter, `finish`: the screen is `GameOver`. Press `r`: the screen is `Menu`, `stage` is still 2 and `max_stage` is 4.
  - `quitting_mid_run_saves_progress`: `staged_app(3)`. Right twice, Enter, then `q` before any tick: `quit` is true, `db.reached("staged")` is 5 and `max_stage` is 5. With `app()`, pressing `5` then `q` leaves `db.progress_rows()` at 0.
  - `game_without_stages_ignores_arrows_and_saves_no_progress`: `app()`. Right leaves `stage` at 1 and `max_stage` at 1. After pressing `5` and `finish`, `db.reached("fake")` is 1 and `db.progress_rows()` is 0.
  - `staged_fake_meets_the_contract`: `testkit::check(&STAGED)`.
  - The existing `fake_meets_the_contract` and the other tests still pass.
- `ui/mod.rs`
  - `menu_stage_picker_dims_arrows_at_the_limits`: draws `staged_app(3)` into a `Terminal<TestBackend>` of 80 by 30 and reads the buffer. The text has `◀ STAGE 1 ▶ of 3` and `←→  stage`. The `◀` cell fg is `Color::Indexed(237)` and the `▶` cell fg is `Color::Indexed(214)`. With `stage = 3` the colors swap. `staged_app(1)` has both arrows at 237.
  - `menu_without_stages_has_no_picker`: the `app()` render has no `STAGE`, and the existing menu tests still pass.
- Snake: every existing test in `games/snake` passes with only the `start(c, 1)` edits.

Commit as `feat(sdk): stage progress with a stage picker on the game menu`.

## Task 2: brick ball crate, stage generator and difficulty levers

Create the crate and the pure stage module. `main.rs` is a stub that only lets the workspace build; Task 4 replaces it.

Files:

- Root `Cargo.toml`: `members = ["crates/sdk", "crates/launcher", "games/snake", "games/brick-ball"]`.
- `Cargo.lock`: gains the `arcade-brick-ball` package entry, written by `cargo build`. Commit it with the task.
- `games/brick-ball/Cargo.toml`:

```toml
[package]
name = "arcade-brick-ball"
version.workspace = true
edition.workspace = true

[dependencies]
anyhow.workspace = true
arcade-sdk.workspace = true
rand.workspace = true
ratatui.workspace = true
```

- `games/brick-ball/src/main.rs`, the stub, replaced in Task 4:

```rust
#[allow(dead_code)] // used by the game from Task 4 on, which removes this allow
mod stage;

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

fn main() -> anyhow::Result<()> { arcade_sdk::run(&ENTRY) }

fn start(c: char, stage: u32) -> Option<Box<dyn Game>>  // Some(Stub { stage }) for '\n' only
struct Stub { stage: u32 }
// Game for Stub: key does nothing, tick returns Over at once with score 0,
// variant "stage {stage}-{stage}", summary "". reached returns stage, so
// testkit::check passes. tick_rate 33 ms. draw draws nothing.
```

- `games/brick-ball/src/stage.rs`: no ratatui, no rusqlite.

Public interface of `stage.rs`, exact:

```rust
pub const COLS: usize = 15;
pub const ROWS: usize = 10;
/// Stages per sawtooth block.
pub const BLOCK: u32 = 5;
pub const MAX_HP: u8 = 5;
/// Ball speed lever at e = 0, px/s. The HUD speed multiplier divides by this.
pub const BASE_SPEED: f64 = 28.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cell { Empty, Brick(u8), Unbreakable }

/// grid[row][col], row 0 at y = 4.
pub type Grid = [[Cell; COLS]; ROWS];

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Levers {
    pub e: u32,
    /// Ball speed in pixels per second.
    pub speed: f64,
    /// Bricks in the stage, breakable and unbreakable.
    pub count: usize,
    /// Paddle width in pixels.
    pub paddle: u32,
    /// Unbreakable bricks wanted. The generator may place fewer, see build.
    pub unbreakable: usize,
}

/// 1 - exp(-e / k).
pub fn g(e: f64, k: f64) -> f64;
/// n - 2 when n > 1 and n % BLOCK == 1, else n.
pub fn effective(n: u32) -> u32;
/// Every lever for effective stage e.
pub fn levers(e: u32) -> Levers;
/// 1 + floor(4 × g(e, 25) + r), capped at MAX_HP. r in [0, 1).
pub fn hp(e: u32, r: f64) -> u8;
/// Seed of stage n in a run.
pub fn stage_seed(run: u64, n: u32) -> u64;
/// Pattern masks, left 8 columns (0 to 7, column 7 is the center). Column c
/// reads PATTERNS[p][r][min(c, 14 - c)], so every mask is mirrored left to right.
pub const PATTERNS: [[&str; ROWS]; 6];
/// True when pattern p has a brick at (c, r).
pub fn mask(p: usize, c: usize, r: usize) -> bool;
/// The layout of stage n of the run with seed `run`.
pub fn build(run: u64, n: u32) -> Grid;
/// True when a flood fill from the open area under the grid, through every
/// cell that is not Unbreakable, reaches every Brick.
pub fn all_reachable(grid: &Grid) -> bool;
```

Lever formulas, exact, in `levers(e)` with `x = e as f64`:

- `speed = BASE_SPEED * (1.0 + 0.7 * g(x, 20.0))`
- `count = (150.0 * (0.45 + 0.40 * g(x, 15.0))).round() as usize`
- `paddle = 9 - (3.0 * g(x, 30.0)).round() as u32`
- `unbreakable = if e < 8 { 0 } else { (count as f64 * 0.10 * g((e - 8) as f64, 20.0)).round() as usize }`. The spec says none before stage 8. `e < 8` is the same test because every `n >= 8` has `e >= 8` and every `n < 8` has `e < 8`. The difficulty depends only on `n` through `e`.

`f64::round` rounds half away from zero, so `150 × 0.45 = 67.5` gives 68 as the spec's table shows. The spec table's column is "Floor (e = 0) → cap": `levers(0)` gives speed 28, count 68, paddle 9, unbreakable 0, and `levers(1000)` gives speed 47.6, count 128, paddle 6, unbreakable 13 (about 10% of 128). No real stage has `e = 0`, because `effective(n) >= 1` for every `n >= 1`; the floor is the formula's value at `e = 0`. Reference values for real stages, which tests may assert:

| n | e | speed | count | paddle | unbreakable |
|---|---|---|---|---|---|
| 1 | 1 | 28.956 | 71 | 9 | 0 |
| 5 | 5 | 32.336 | 85 | 9 | 0 |
| 6 | 4 | 31.553 | 82 | 9 | 0 |
| 10 | 10 | 35.712 | 97 | 8 | 1 |
| 20 | 20 | 40.390 | 112 | 8 | 5 |
| 1000 | 1000 | 47.6 | 128 | 6 | 13 |

`stage_seed(run, n)` is `run ^ (n as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)`. Plan decision.

Pattern masks, plan decision. Each string is columns 0 to 7. The full row mirrors columns 0 to 6 after column 7. Each mask has at most 71 cells, the brick count at stage 1, so a mask always fits whole.

```rust
pub const PATTERNS: [[&str; ROWS]; 6] = [
    // 0 rows, 65 cells
    [".#######", "........", ".#######", "........", ".#######",
     "........", ".#######", "........", ".#######", "........"],
    // 1 pyramid, 64 cells
    ["........", "........", ".......#", "......##", ".....###",
     "....####", "...#####", "..######", ".#######", "########"],
    // 2 diamond, 56 cells
    ["........", ".......#", ".....###", "...#####", ".#######",
     ".#######", "...#####", ".....###", ".......#", "........"],
    // 3 checker, 68 cells
    ["#.#.#.#.", ".#.#.#.#", "#.#.#.#.", ".#.#.#.#", "#.#.#.#.",
     ".#.#.#.#", "#.#.#.#.", ".#.#.#.#", "#.#.#.#.", "........"],
    // 4 columns, 70 cells
    ["..##..##", "..##..##", "..##..##", "..##..##", "..##..##",
     "..##..##", "..##..##", "..##..##", "..##..##", "..##..##"],
    // 5 invader, 46 cells
    ["........", "....#...", ".....#..", "....####", "...##.##",
     "..######", "..#.####", "..#.#...", ".....##.", "........"],
];
```

`build(run, n)`, in this order. The order of random draws is fixed so a seed always gives the same grid:

1. `rng = StdRng::seed_from_u64(stage_seed(run, n))`, `lv = levers(effective(n))`.
2. `p = rng.random_range(0..6)` picks the pattern.
3. Fill: every cell where `mask(p, c, r)` is set is filled. Then, while fewer than `lv.count` cells are filled, fill empty cells in fill order. Fill order is rows top first (row 0 to 9), and inside a row columns center outward: `const FILL_ORDER: [usize; COLS] = [7, 6, 8, 5, 9, 4, 10, 3, 11, 2, 12, 1, 13, 0, 14];`. Plan decision: center outward keeps the layout near symmetric.
4. HP: for every filled cell in row-major order (row 0 col 0 first), `Cell::Brick(hp(lv.e, rng.random::<f64>()))`.
5. Unbreakable: candidates are all filled cells in row-major order, shuffled with a Fisher-Yates shuffle that draws `rng.random_range(0..=i)`. Plan decision. Walk the candidates while fewer than `lv.unbreakable` are placed. Set the candidate to `Unbreakable`. If `all_reachable` is false, put its `Brick(h)` back and skip it. The filled count never changes, so bricks plus unbreakables equal `lv.count`.

`all_reachable(grid)`: a breadth-first flood fill, 4 neighbours, over a 12 by 15 grid. Row 0 is the open lane above the bricks (`y` 0 to 3), rows 1 to 10 are brick rows 0 to 9, and row 11 is the open area under the bricks. Both lane rows are always passable. A brick row cell is passable unless it is `Unbreakable`. The fill starts from all 15 cells of row 11. The result is true when every `Brick` cell was visited. The brick grid spans the full 60 pixel width, so there is no path around its sides.

`hp(e, r)`: `(1 + (4.0 * g(e as f64, 25.0) + r).floor() as u8).min(MAX_HP)`. At `e = 0` it is 1 for every `r`. At the cap `g` is 1 in `f64` (from about `e = 1000`), so it is 5 for every `r`. The `min` is the spec's "never above 5".

Tests in `stage.rs`:

- `levers_match_the_spec_floor_and_cap`: `levers(0)` has speed `BASE_SPEED` within 1e-9, count 68, paddle 9 and unbreakable 0. `levers(1000)` has speed 47.6 within 1e-9, count 128, paddle 6 and unbreakable 13.
- `levers_stay_between_floor_and_cap_and_rise_with_e`: for `e` in 0 to 500, `28.0 <= speed <= 47.6 + 1e-9`, `68 <= count <= 128`, `6 <= paddle <= 9`, `unbreakable <= 13`. For each consecutive pair, speed and count do not fall and paddle does not rise. Unbreakable does not fall for `e >= 8`.
- `levers_match_the_reference_table`: the six rows of the table above, speed within 0.001.
- `sawtooth_eases_the_first_stage_of_a_block`: `effective(1) == 1`, `effective(5) == 5`, `effective(6) == 4`, `effective(7) == 7`, `effective(11) == 9`.
- `hp_is_all_one_at_the_floor_and_reaches_five_at_the_cap`: for `r` from 0.0 to 0.99 in steps of 0.01, `hp(0, r) == 1`. `hp(1, 0.0) == 1` and `hp(1, 0.9) == 2`. `hp(25, 0.0) == 3` and `hp(25, 0.5) == 4`. `hp(1000, 0.0) == 5` and `hp(1000, 0.999) == 5`. For seeds 0 to 9, the bricks of `build(seed, 200)` include a `Brick(5)` and every `Brick(h)` has `h` 4 or 5. A coefficient of 3 instead of 4 fails every cap assertion.
- `masks_read_the_mirrored_column_and_fit_stage_one`: every row string is 8 characters of `#` and `.`. The cell counts of the six masks are exactly 65, 64, 56, 68, 70 and 46, each at most `levers(effective(1)).count`. Known cells: pyramid `mask(1, 0, 9)` and `mask(1, 14, 9)` are set; `mask(1, 7, 2)` is set and `mask(1, 6, 2)` and `mask(1, 8, 2)` are not; invader `mask(5, 4, 1)` and `mask(5, 10, 1)` are set and `mask(5, 7, 1)` is not. A `mask` that indexes the string with the wrong column, for example `c.min(7)`, fails the right-half cells.
- `same_seed_gives_the_same_layout`: `build(42, 7) == build(42, 7)` and `build(42, 23) == build(42, 23)`. At least one of `build(s, 7)` for `s` in 0 to 9 differs from `build(42, 7)`.
- `brick_count_matches_the_lever`: for `n` in `[1, 2, 5, 6, 9, 10, 15, 20, 37, 60, 200]` and seeds 0 to 19, the count of non-empty cells is `levers(effective(n)).count`, and the unbreakable cells are at most `levers(effective(n)).unbreakable`.
- `every_breakable_brick_is_reachable`: for seeds 0 to 199 and `n` in `[8, 10, 13, 20, 40, 80, 200]`, `all_reachable(&build(seed, n))`.
- `reachability_finds_a_sealed_brick`: a hand-made grid, all other cells `Empty`, with `Brick(1)` at (7, 1) and `Unbreakable` at (7, 0), (6, 1), (8, 1) and (7, 2) gives false. The pocket is sealed on all four sides, so neither the lane above nor the area below reaches it. Removing the `Unbreakable` at (7, 2) gives true.
- `no_unbreakable_before_stage_8`: for `n` in 1 to 7 and seeds 0 to 49, no cell is `Unbreakable`.
- `unbreakables_appear_by_stage_20`: for seeds 0 to 19, `build(seed, 20)` holds at least one `Unbreakable`.

Commit as `feat(brick-ball): crate skeleton with the stage generator and difficulty levers`.

## Task 3: rules and physics

Create `games/brick-ball/src/board.rs`, the rules of the game with no terminal code, like snake's `board.rs`. In `main.rs` add `#[allow(dead_code)] mod board;` with the same comment as `stage`. The stub is unchanged. `board.rs` imports only `std`, `rand` and `crate::stage`.

Public interface, exact:

```rust
use std::time::Duration;

pub const W: f64 = 60.0;          // pixels
pub const H: f64 = 44.0;
pub const BRICK_W: f64 = 4.0;
pub const BRICK_TOP: f64 = 4.0;   // y of brick row 0
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
pub const CAPSULE_SPEED: f64 = 20.0;  // px/s
pub const WIDE_PX: u32 = 3;
pub const SLOW_FACTOR: f64 = 0.75;
/// Timers in ticks: whole ticks in the duration, rounded down.
pub const WIDE_TICKS: u32 = ticks(Duration::from_secs(15));  // 454
pub const SLOW_TICKS: u32 = ticks(Duration::from_secs(10));  // 303
pub const CLEAR_TICKS: u32 = ticks(Duration::from_secs(2));  // 60
/// Hold ticks for the paddle step to rise from 1 to 3 pixels.
pub const HOLD_RAMP: u32 = 6;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ball { pub x: f64, pub y: f64, pub dx: f64, pub dy: f64 } // (dx, dy) is a unit vector

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Power { Wide, Slow, Multi }
impl Power { pub fn letter(self) -> char; } // 'W', 'S', 'M'

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Capsule { pub kind: Power, pub x: f64, pub y: f64 }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    /// Ball waits on the paddle for space.
    Serve,
    Play,
    /// Stage cleared, counting down to the next stage.
    Clear { ticks: u32 },
    /// Last life lost: half-periods of the paddle blink left.
    Dying { left: u8 },
    /// The blink finished. The adapter returns Status::Over.
    Over,
}

pub struct Board {
    pub run_seed: u64,
    pub start: u32,
    pub stage: u32,
    pub reached: u32,
    pub levers: crate::stage::Levers,
    pub cells: crate::stage::Grid,
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
    hold: u32,      // ticks of the current paddle hold
    hold_dir: i32,
    pressed: i32,   // direction of a single press since the last tick, -1, 0 or 1
    rng: rand::rngs::StdRng,
}

// Private methods that tests in board.rs call directly:
// fn hit(&mut self, c: usize, r: usize)  one hit on cell (c, r): score, break, capsule roll, clear check
// fn apply(&mut self, p: Power)           the effect of a caught capsule

/// d.as_millis() / TICK.as_millis(), as u32. Private.
const fn ticks(d: Duration) -> u32;
/// Unit direction leaving the paddle for t in -1..=1, with a = MAX_ANGLE_DEG.to_radians() × t:
/// (sin a, -cos a).
pub fn bounce_dir(t: f64) -> (f64, f64);
/// Sub-steps for a move of `dist` pixels: max(1, ceil(dist / MAX_STEP)).
pub fn substeps(dist: f64) -> u32;
/// Paddle pixels moved on hold tick k (1 based):
/// 1 + 2 × (min(k, HOLD_RAMP) - 1) / (HOLD_RAMP - 1).
pub fn paddle_step(k: u32) -> f64;
/// Whole seconds shown for `ticks` left: ceil(ticks × TICK.as_millis() / 1000).
pub fn secs(ticks: u32) -> u32;

impl Board {
    /// Stage `start` of a run: layout from stage::build(run_seed, start), lives
    /// START_LIVES, paddle centered, ball on the paddle, reached = start,
    /// rng = StdRng::seed_from_u64(run_seed).
    pub fn new(run_seed: u64, start: u32) -> Board;
    /// Paddle width in pixels: levers.paddle, plus WIDE_PX while wide > 0.
    pub fn paddle_w(&self) -> f64;
    /// Paddle left edge: paddle_cx - paddle_w / 2.
    pub fn paddle_left(&self) -> f64;
    /// Ball speed in px/s: levers.speed, times SLOW_FACTOR while slow > 0.
    pub fn ball_speed(&self) -> f64;
    /// ball_speed / stage::BASE_SPEED, for the HUD.
    pub fn speed_multiplier(&self) -> f64;
    /// Space: Serve becomes Play with the launch tilt. No-op in other phases.
    pub fn launch(&mut self);
    /// A single key press of direction dir (-1 or 1). The next tick moves the
    /// paddle 1 pixel even when no hold is reported. Ignored while dying.
    pub fn press(&mut self, dir: i32);
    /// One tick. dir is -1, 0 or 1 for the paddle. Ignored while dying.
    pub fn tick(&mut self, dir: i32);
    pub fn over(&self) -> bool;                // phase == Over
    pub fn breakable_left(&self) -> usize;
    /// First multiple of LIFE_EVERY above the current stage:
    /// (stage / LIFE_EVERY + 1) × LIFE_EVERY.
    pub fn next_life_stage(&self) -> u32;
    pub fn variant(&self) -> String;            // "stage {start}-{reached}"
    pub fn summary(&self) -> String;            // "▬ {bricks} bricks"
}
```

Every value that has a constant above is written through that constant, never as its literal: `DT` comes from `TICK`, the tick counts and `secs` from `TICK.as_millis()`, `paddle_step` uses `HOLD_RAMP`, `bounce_dir` uses `MAX_ANGLE_DEG`, and the geometry below uses `BRICK_W`, `BRICK_TOP` and `PADDLE_Y`. The numbers in this section are what those constants evaluate to. The same holds for `START_LIVES`, `MAX_LIVES` and `LIFE_EVERY`. So every item of this interface is used and Task 4 deletes nothing from it.

`Board::new` seeds `rng` with `StdRng::seed_from_u64(run_seed)`. The layout comes from `stage::build`, which seeds its own generator from `stage_seed`, so the board's draws (launch tilt, capsule drops and kinds) never change a layout. A fixed `run_seed` gives the same sequence of board draws, which the capsule test depends on.

Geometry: a ball is a point. It lights pixel `(floor(x), floor(y))`. Brick `(c, r)` covers `x` in `[BRICK_W × c, BRICK_W × (c + 1))` and `y` in `[BRICK_TOP + r, BRICK_TOP + r + 1)`. The paddle covers `x` in `[paddle_left, paddle_left + paddle_w)` at pixel row `PADDLE_Y`. `paddle_cx` is clamped to `[paddle_w / 2, W - paddle_w / 2]` after every move and every width change, so the paddle grows and shrinks around its center. Plan decision.

Behaviour of `tick(dir)` by phase:

- `Over`: nothing.
- `Dying { left }`: `left > 1` sets `left - 1`, otherwise the phase becomes `Over`. `dir` is ignored and nothing moves. The adapter ticks at `BLINK` in this phase, so six ticks take six half-periods of 200 ms.
- `Clear { ticks }`: `ticks > 1` sets `ticks - 1`. Otherwise the next stage starts:
  - `stage += 1` and `reached = max(reached, stage)`.
  - `levers = levers(effective(stage))`, `cells = build(run_seed, stage)` and `stage_points = 0`.
  - Then serve.
- `Serve`: move the paddle, then put the one ball at `(paddle_cx, PADDLE_Y - 0.5)`.
- `Play`, in this order:
  1. Move the paddle.
  2. Count `wide` and `slow` down by 1 when above 0. When `wide` reaches 0, re-clamp `paddle_cx`.
  3. Move the capsule.
  4. Move every ball. A ball that clears the stage sets `phase = Clear` and empties `balls`, see "Stage clear" below. The loop over balls checks the phase before each ball and stops as soon as it is no longer `Play`, so a loop by index never reads past the end of the emptied `balls`. If the phase is no longer `Play` after this step, the tick ends here, so steps 5 and 6 do not run and a clear never costs a life.
  5. Remove lost balls.
  6. If no ball is left, lose a life.

Paddle movement: first, when `dir == 0`, `dir = pressed`. Then `pressed = 0`. So a single press since the last tick moves the paddle as the first tick of a hold, 1 pixel, even in a terminal that reports no hold until key repeat starts; while a hold is reported, the press adds nothing on top. `press(d)` sets `pressed = d` in every phase except `Dying` and `Over`. Then: when `dir != 0` and `dir == hold_dir`, `hold += 1`. When `dir != 0` and it differs, `hold = 1` and `hold_dir = dir`. When `dir == 0`, `hold = 0` and `hold_dir = 0`. A nonzero `dir` moves `paddle_cx` by `dir × paddle_step(hold)`, then clamps, so the paddle stops at the walls.

Serve (used by `new`, a lost life and a new stage):

- `phase = Serve` and `balls = [Ball { x: paddle_cx, y: PADDLE_Y - 0.5, dx: 0.0, dy: -1.0 }]`.
- `capsule = None`, so a falling capsule is removed.
- Operator ruling: `wide` and `slow` are not reset on serve, after a lost life or at a new stage. A running W or S carries on.
- Plan decision: the timers count down only in `Play` (step 2), so they hold still while the ball waits on the paddle and during the `Clear` countdown.

`launch`: in `Serve`, draw `a = rng.random_range(-LAUNCH_TILT_DEG..LAUNCH_TILT_DEG)` in degrees. The ball direction becomes `(sin a, -cos a)` and the phase becomes `Play`.

Ball movement, per ball per tick:

- `dist = ball_speed() × DT` and `n = substeps(dist)`. Each sub-step moves `(dx, dy) × dist / n`. Sub-steps stop early when the phase leaves `Play`, because the stage was cleared.
- After each sub-step, from the old position `(ox, oy)` to the new `(x, y)`, check in this order:
  1. Left wall: `x < 0` sets `x = -x` and `dx = |dx|`. Right wall: `x >= W` sets `x = 2W - x - 1e-9` and `dx = -|dx|`. Ceiling: `y < 0` sets `y = -y` and `dy = |dy|`.
  2. Brick: if `floor(y) - BRICK_TOP` is in `0..ROWS` and the cell at column `floor(x / BRICK_W)` is not `Empty`, the ball entered it.
     - The axis is y when `floor(oy) != floor(y)`, otherwise x. Plan decision: when both the pixel row and the brick column changed, the axis is y, because bricks are 1 pixel tall.
     - The ball goes back to `(ox, oy)` and the component on that axis is negated.
     - A `Brick(h)` takes one hit through `hit`: score and `stage_points` gain `10 + stage`. When `h == 1` the cell becomes `Empty`, `bricks += 1` and a capsule may drop, otherwise it becomes `Brick(h - 1)`.
     - The clear check runs only when a `Brick` breaks: then, if `breakable_left() == 0`, the stage is clear, see below. `Unbreakable` only reflects. It does not call `hit` and never runs the clear check.
  3. Paddle: if `dy > 0`, `floor(oy) < PADDLE_Y`, `floor(y) == PADDLE_Y` and `paddle_left <= x < paddle_left + paddle_w`, the ball goes back to `(ox, oy)`. Its direction becomes `bounce_dir(t)` with `t = clamp(2 × (x - paddle_left) / paddle_w - 1, -1, 1)`. Paddle motion does not enter the angle.
- A ball with `y >= H` has passed pixel row 43 and is lost. Plan decision: the spec's "y passes 43" means it left the last pixel row.

Lives:

- When the last ball is lost in `Play`, `lives -= 1`. With `lives > 0` the board serves again. With `lives == 0`, `balls` is cleared, `capsule = None` and the phase becomes `Dying { left: BLINKS }`.
- Losing one of several balls costs nothing.

Capsules:

- When a brick breaks and `capsule` is `None`, `rng.random_bool(CAPSULE_CHANCE)` decides a drop. On a drop the kind is `rng.random_range(0..3)` over Wide, Slow, Multi (plan decision: equal chances), placed at the brick center `(BRICK_W × c + BRICK_W / 2, BRICK_TOP + r + 0.5)`.
- Each `Play` tick the capsule moves `y += CAPSULE_SPEED × DT`.
- On the tick its `y` first reaches `PADDLE_Y` (it was below `PADDLE_Y` before the move), it is caught if `paddle_left <= x < paddle_left + paddle_w`. Plan decision: the catch is checked only on that tick.
- A caught capsule is removed and its effect applies at once. A capsule with `y >= H` is gone with no effect.

Effects:

- Wide: `wide = WIDE_TICKS`, then `paddle_cx` re-clamps.
- Slow: `slow = SLOW_TICKS`.
- Catching the same kind again sets the timer back to full.
- Multi: for each ball present before the split, in order, add copies while `balls.len() < MAX_BALLS`. Copy one has direction `(-sin 45°, s × cos 45°)` and copy two `(sin 45°, s × cos 45°)`, where `s = dy.signum()` of the source ball. Both start at the source position. Plan decision: copies leave at fixed ±45° from vertical in the source ball's vertical direction, which stays inside the 60° band. 1 ball gives 3, 3 give 9, 5 give 9.

Stage clear:

- When the last breakable brick breaks, `score += 100 × stage`. When `stage % LIFE_EVERY == 0`, `lives = min(lives + 1, MAX_LIVES)`.
- `balls` is cleared, `capsule = None` and `phase = Clear { ticks: CLEAR_TICKS }`. Unbreakable bricks do not need clearing.
- The adapter shows `secs(ticks)`, so the countdown reads 2 then 1.

Tests in `board.rs`. A helper `empty(stage)` returns `Board::new(1, stage)` with every cell `Empty`, `phase = Play`, and one ball at `(30.0, 20.0)` moving straight up. That ball stays in play, because it comes back to the paddle center and leaves straight up again, so a test can tick many times without losing a life. A clear only happens after a hit, so the empty grid does not end the stage. Tests that need other balls replace `balls`. Score, clear and capsule tests call `hit` and `apply` directly, except `clearing_through_tick_costs_no_life`, which goes through `tick`.

- Paddle angle:
  - `paddle_angle_is_60_degrees_times_t`: `bounce_dir(0.0)` is `(0, -1)`, `bounce_dir(1.0)` is `(0.866, -0.5)` and `bounce_dir(-1.0)` is `(-0.866, -0.5)`, within 1e-3.
  - `ball_at_the_left_edge_leaves_at_minus_60`: a ball at `(paddle_left + 0.01, 41.9)` moving down. After one tick its direction matches `bounce_dir(-1.0)` within 0.02.
- Sub-steps and tunnelling:
  - `substeps_never_exceed_half_a_pixel`: for `dist` from 0.0 to 100.0 in steps of 0.01, `substeps(dist) >= 1` and `dist / substeps(dist) <= 0.5`.
  - `cap_speed_ball_never_tunnels_a_one_pixel_brick`: `levers.speed = levers(1000).speed` and the only cell is `Brick(1)` at (7, 5), pixels x 28 to 31, y 9. For angles -50, -30, -10, 0, 10, 30 and 50 degrees and start heights `y0 = 30.0 + 0.1 × k` for `k` in 0 to 15, a ball starts at `(30 - (y0 - 9.5) × tan a, y0)` with direction `(sin a, -cos a)`, so its path crosses (30, 9.5) inside the brick. Tick until the cell is `Empty` or the ball has `y < 9`. Assert the cell is `Empty` before the ball first has `y < 9`, so a ball that passes row 9 without a hit fails. A guard of 60 ticks fails the test if neither happens. The spread of start heights over one pixel makes some runs skip row 9 when sub-stepping is removed (a cap speed tick moves 1.57 px, more than 1 px in y for every listed angle), so the test fails without sub-steps.
- Walls and bricks:
  - `walls_and_ceiling_reflect`: a ball at `(0.2, 20.0)` moving `(-0.6, -0.8)` has `dx > 0` after a tick. A ball at `(30.0, 0.2)` moving `(0, -1)` has `dy > 0`.
  - `brick_reflects_on_the_entry_axis`: three cases, each from a fresh `empty(1)`, and each grid holds a `Brick(1)` at (0, 0) that is never hit, so no case clears the stage and empties `balls` before the direction is read (as in the capsule test).
    - From below: a ball moving up into `Brick(2)` at (7, 5). After the tick `dy` has flipped, `dx` is kept and the cell is `Brick(1)`.
    - From the side: a fresh `Brick(2)` at (7, 5) and a ball at `(27.9, 9.5)` moving `(0.866, 0.5)`. After the tick `dx` has flipped and the cell is `Brick(1)`.
    - `Unbreakable` at (7, 5): a ball moving up into it reflects, the cell stays `Unbreakable` and phase is still `Play`.
- Lives:
  - `last_ball_lost_costs_a_life_and_serves_again`: lives 3, one ball at `(30.0, 43.9)` moving down. After a tick, lives is 2, phase is `Serve` and one ball sits at `y` `PADDLE_Y - 0.5`.
  - `losing_one_of_several_balls_costs_nothing`: two balls, one about to leave the bottom. After a tick, lives is 3, one ball is left and phase is `Play`.
  - `last_life_blinks_six_half_periods_then_over`: lives 1, last ball lost, phase `Dying { left: 6 }`. Five more ticks with `dir = 1` are not over and do not move `paddle_cx`. The sixth gives `over()`.
- Score and extra lives:
  - `hits_score_ten_plus_stage_and_clear_scores_hundred_times_stage`: stage 3, a single `Brick(2)`. The first hit gives score 13, the second gives score 26 + 300, `stage_points` 26, `bricks` 1 and phase `Clear { ticks: 60 }`.
  - `clearing_through_tick_costs_no_life`: `empty(3)` with lives 3, the only cell `Brick(1)` at (7, 5), and one ball at `(30.0, 10.2)` moving straight up. After one tick the cell is `Empty`, lives is still 3, `balls` is empty and phase is `Clear { ticks: 60 }`. This test goes through `tick`, so it fails if steps 5 and 6 run after a clear.
  - `clearing_a_multiple_of_five_adds_a_life_up_to_five`: clearing stage `LIFE_EVERY` with lives 3 gives 4, and with lives `MAX_LIVES` gives `MAX_LIVES`. Clearing stage `LIFE_EVERY - 1` with lives 3 stays 3. `next_life_stage()` is 5 at stages 1 and 4 and 10 at stage 5.
  - `clear_counts_down_then_starts_the_next_stage`: after a clear at stage 3, 59 ticks stay in `Clear`. The 60th tick gives stage 4, reached 4, phase `Serve`, and a non-empty cell count of `levers(effective(4)).count`. `secs(60) == 2`, `secs(31) == 2`, `secs(30) == 1`.
  - `new_board_has_reached_the_start_stage`: `Board::new(9, 7)` has reached 7, lives 3 and phase `Serve`. `variant()` is `"stage 7-7"`. With `bricks = 12`, `summary()` is `"▬ 12 bricks"`.
- Power-ups:
  - `wide_and_slow_timers_refresh_on_a_second_catch`: catching W gives `wide == 454` and `paddle_w == levers.paddle + 3`. After 100 ticks it is 354. A second W gives 454 again. The same holds for S with 303, and `ball_speed == levers.speed × 0.75` while it runs.
  - `serve_keeps_power_up_timers`: `wide = 100`, `slow = 50`, a falling capsule, and the only ball at `(30.0, 43.9)` moving down. After a tick, phase is `Serve`, `capsule` is `None`, `wide` is 99 and `slow` is 49. Ten more ticks in `Serve` leave them at 99 and 49.
  - `multi_splits_every_ball_into_three_up_to_nine`: 1 ball gives 3, 3 give 9, 5 give 9, 9 give 9.
  - `capsules_drop_about_eight_percent_and_one_at_a_time`: `empty(1)` with `Brick(1)` at (7, 5) and a second `Brick(1)` at (0, 0) that is never hit, so `breakable_left()` stays above 0 and `hit` never clears the stage (a clear sets `capsule = None` and would erase every drop). Before each break, set (7, 5) back to `Brick(1)`, then call `hit(7, 5)`. `empty(1)` builds the board with `run_seed` 1, so the rng sequence is fixed. 10 000 breaks with the capsule set to `None` after each give between 650 and 950 drops. With a capsule present, 1 000 breaks leave it equal to the first one.
  - `capsule_caught_over_the_paddle_or_lost_past_the_bottom`: a capsule at `(paddle_cx, 41.9)` is caught on the next tick. A capsule at `(2.0, 41.9)` with the paddle centered is not caught, is `None` once `y >= 44`, and has no effect.
- Paddle:
  - `paddle_accelerates_from_one_to_three_and_stops_at_walls`: in `Serve` from the center, six ticks with `dir = 1` move by 1.0, 1.4, 1.8, 2.2, 2.6 and 3.0. The seventh moves 3.0. A tick with `dir = 0` and then `dir = 1` moves 1.0 again. A hundred ticks with `dir = 1` leave `paddle_cx == W - paddle_w / 2`. The serve ball follows the paddle.
  - `single_press_moves_one_pixel_on_the_next_tick`: in `Serve` from the center, `press(1)` then `tick(0)` moves by 1.0, and a second `tick(0)` does not move. `press(-1)` then `tick(0)` moves by -1.0. `press(1)` then `tick(1)` moves by 1.0, not 2.0. In `Dying`, `press(1)` then `tick(0)` does not move.

Commit as `feat(brick-ball): rules and physics module`.

## Task 4: game adapter, drawing, HUD, overlays and thumbnail

Replace the stub in `main.rs` with the real `Game`, add `draw.rs`, the thumbnail, and the arcade contract test. Remove both `#[allow(dead_code)]` attributes from `main.rs`. Any item that is still unused outside tests is then deleted or moved under `#[cfg(test)]`, so clippy passes with no allow.

Files:

- `games/brick-ball/src/main.rs`, rewritten
- `games/brick-ball/src/draw.rs`, new

`main.rs`, exact:

```rust
mod board;
mod draw;
mod stage;

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

fn main() -> anyhow::Result<()> { arcade_sdk::run(&ENTRY) }

/// Enter starts a run at the chosen stage with a fresh random run seed.
fn start(c: char, stage: u32) -> Option<Box<dyn Game>>; // Some only for '\n', BrickBall::new(rand::random(), stage)

struct BrickBall {
    board: Board,
    paused: bool,
}

impl BrickBall { fn new(run_seed: u64, stage: u32) -> BrickBall; }
```

Plan decision: the mockup's thumbnail uses `S` (light terracotta 209) for the paddle. The spec says a terracotta paddle and the in-game paddle is 173, so the art uses `s` (terracotta 173).

`Game for BrickBall`:

- `key(key)`: while `board.phase` is `Dying` or `Over`, every key is ignored. `Char('p')` toggles `paused`. While paused, other keys are ignored. `Char(' ')` calls `board.launch()`. `Left` and `Char('a')` call `board.press(-1)`. `Right` and `Char('d')` call `board.press(1)`.
- `tick(input)`:
  - When paused, return `Running` and change nothing, so the `Clear` countdown and the power-up timers also stop.
  - Otherwise `dir` is -1 when `input.held(Left)` or `input.held(Char('a'))`, 1 for `Right` or `Char('d')`, and else 0. Then call `board.tick(dir)`. The spec's single press rule (1 pixel on the next tick, for terminals without release events) lives in `Board::press` and `Board::tick`, see Task 3.
  - When `board.over()`, return `Status::Over(Outcome { score: board.score, variant: board.variant(), summary: board.summary() })`. Otherwise return `Running`.
- `reached()`: `board.reached`, from the rules module. The SDK reads it when the run ends and when the player quits with `q` during the run.
- `tick_rate()`: `board::BLINK` while the phase is `Dying`, else `board::TICK`.
- `draw(frame, ctx)`: `draw::draw(frame, self, ctx)`.

`draw.rs` layout:

```rust
pub const BOARD_W: u16 = 62;  // 60 cells + border
pub const BOARD_H: u16 = 24;  // 22 cells + border
pub const HUD_W: u16 = 16;    // the spec's 16 columns, border included
pub const TOTAL_W: u16 = BOARD_W + 1 + HUD_W; // 79

pub fn draw(frame: &mut Frame, game: &BrickBall, ctx: &DrawCtx);
```

Drawing behaviour:

- Placement: `area = ui::centered(frame.area(), TOTAL_W, BOARD_H)`. The board panel is at `area.x` and the HUD at `area.x + BOARD_W + 1`. Both use `ui::panel`, with `theme::CHROME` live and `theme::CHROME_DIM` dimmed. The board panel has the top title `" brick ball "` in `theme::MUTED` (`CHROME_DIM` dimmed), as snake does.
- Dim: the frame is dimmed when `ctx.dim || game.paused || phase is Clear`. Plan decision: the board draws dimmed under the PAUSED and STAGE CLEAR modals, as under the SDK modals and in the mockup.
- Pixels: cell `(cx, cy)` of the board's inner area shows pixel `(cx, 2cy)` on top and `(cx, 2cy + 1)` below.
  - Both pixels field: `" "` with bg 233.
  - Top only: `▀` with fg top and bg 233.
  - Bottom only: `▄` with fg bottom and bg 233.
  - Both: `▀` with fg top and bg bottom.
- Pixel colors, later rules win:
  1. Field 233.
  2. Bricks: for `y` in `BRICK_TOP` to `BRICK_TOP + ROWS - 1` and the cell at `(x / BRICK_W, y - BRICK_TOP)`, with the `board` constants taken as `usize`. `Brick(h)` uses the light color of its HP pair for `x % BRICK_W` below `BRICK_W - 1` and the dark color for `x % BRICK_W == BRICK_W - 1`. `Unbreakable` uses 245 and 239 the same way. `const HP_COLORS: [(u8, u8); 5] = [(223, 180), (215, 173), (209, 167), (167, 131), (124, 88)];` and `const UNBREAKABLE: (u8, u8) = (245, 239);`.
  3. Paddle 173: pixel row `PADDLE_Y`, pixel `x` lit when `paddle_left <= x + 0.5 < paddle_left + paddle_w`. In `Dying` it is drawn in `theme::DEAD` (131) when `left % 2 == 1` and not drawn otherwise, as snake's blink does.
  4. Balls 255 at `(floor(x), floor(y))`.
- Capsule: one cell at `(floor(x), floor(y) / 2)` holding its letter, fg 233 bold on bg 45 (`W`), 82 (`S`) or 201 (`M`).
- Dimmed: every lit pixel and the capsule chip use `theme::DIM_C` (237). The field stays 233.

HUD rows, inner area `hud_inner` with text at `x = hud_inner.x + 1`, from the mockup. Labels are `theme::AMBER`. Dimmed, every style is `theme::DIM`.

| inner row | content |
|---|---|
| 1 | `SCORE` |
| 2 | the score, `theme::ACCENT_HI` |
| 4 | `STAGE` |
| 5 | `format!("{:<4}", stage)` in `theme::TEXT_BOLD`, then `stage::BLOCK` `▮` marks: the first `(stage - 1) % stage::BLOCK + 1` in `theme::AMBER`, the rest in `theme::METER_OFF` |
| 7 | `LIVES` |
| 8 | `●` for each life then `○` up to `board::MAX_LIVES`, separated by spaces, for example `● ● ● ○ ○`. `●` is `theme::ACCENT_HI` and `○` is `theme::METER_OFF` |
| 10 | `BEST` |
| 11 | `{name} {score}` with the name in `theme::GOLD` and the score in `theme::TEXT`, or `none yet` in `theme::MUTED` |
| 13 | `POWER` |
| 14, 15 | one line per running timer, W first then S: the letter chip `" W "` (fg 233 bold on 45, or on 82 for S), a space, a 5 mark bar with `ticks × 5 / max` rounded up as `▰` in the chip color and the rest `▱` in `theme::METER_OFF`, a space, then `"{secs}s"` in `theme::MUTED`. With neither running, `none` in `theme::DIM` |
| 17 | `SPEED` |
| 18 | `format!("{:.2}×", board.speed_multiplier())` in `theme::TEXT` |

Plan decisions for the HUD:

- `M` is instant and has no line under POWER, because the spec lists POWER lines with a bar and seconds left.
- Numbers are plain digits with no thousands separators, matching the SDK top table.

Key chips: live only, in the HUD's bottom border, set with `Block::title_bottom` on the HUD panel. The line is `←→`, `␣`, `p`, `q`, each in `theme::CHIP` with no padding, separated by single spaces and centered: `←→ ␣ p q`. Plan decision: the spec makes the HUD 16 columns including its border, so its inner width is 14. `ui::chips` pads each key and puts two spaces between chips, which needs 21 columns, so it does not fit. The compact `←→ ␣ p q` line is built in `draw.rs` instead. Dimmed, there are no chips.

Overlays, drawn after the board and HUD, centered on the frame with `ui::centered`. Each draws `ratatui::widgets::Clear` first, then `ui::modal`:

- Paused: a 30 by 7 `ui::modal("PAUSED", theme::ACCENT_HI, &[("p", "resume"), ("q", "quit")])`. Inner row 1 holds `stage {n}  ·  {score}` centered in `theme::MUTED`.
- Stage clear, in phase `Clear { ticks }`: a 36 by 10 `ui::modal(&format!("STAGE {n} CLEAR"), theme::GOLD, &[])`. Inner rows, labels in `theme::MUTED` padded to 12 and values right-aligned to the inner width minus 2:
  - Row 1: `bricks` and `+{stage_points}`.
  - Row 2: `clear bonus` and `+{100 × n}`.
  - Row 3: `score` and `{score}` in `theme::ACCENT_HI`.
  - Row 5, centered: `next life at stage {next_life_stage()}` in `theme::MUTED`.
  - Row 7, centered: `STAGE {n + 1} IN {secs(ticks)}` in `theme::AMBER`.
- Paused during `Clear`: the STAGE CLEAR modal draws first and the PAUSED modal draws on top of it. Plan decision: the pause is the newer state and holds the keys that apply (`p` resume, `q` quit), so it is the one the player must see. The PAUSED modal fits inside the STAGE CLEAR modal, so drawing the other way round would hide it.
- Death has no modal. The spec's death is the paddle blink, after which the SDK's game over modal follows.

Tests in `main.rs`:

- `meets_the_arcade_contract`: `arcade_sdk::testkit::check(&ENTRY)`.
- `starts_only_on_enter_at_the_chosen_stage`: `start('\n', 3)` is `Some`, while `start(' ', 1)` and `start('1', 1)` are `None`. `BrickBall::new(5, 3).board.stage == 3`.
- `pause_freezes_the_board`: after `p`, five ticks with Right held leave `paddle_cx` and the balls unchanged. After `p` again, a tick moves the paddle.
- `keys_are_ignored_while_dying`: in `Dying`, `p` does not pause and Right does not move the paddle.
- `over_carries_score_variant_summary_and_reached`: a board set to `Dying { left: 1 }` with score 40, start 2, reached 4 and bricks 3. One tick returns `Over` with `(40, "stage 2-4", "▬ 3 bricks")`, and `reached()` is 4.
- `reached_reports_the_board_during_a_run`: `BrickBall::new(5, 3)` has `reached()` 3. With `board.reached = 6`, it is 6.
- `tap_moves_the_paddle_one_pixel`: a `Right` key press with no hold, then one tick, moves `paddle_cx` by 1.0.

Tests in `draw.rs`, with `testkit::render` for text and a `Terminal<TestBackend>` buffer for colors:

- `renders_hud_labels_and_paddle_at_80_by_24`: `BrickBall::new(1, 1)` drawn at 80 by 24 with best `ABC 42`. The text has `SCORE`, `STAGE`, `LIVES`, `● ● ● ○ ○`, `BEST`, `ABC 42`, `POWER`, `none`, `SPEED`, `1.03×`, `←→ ␣ p q` and `brick ball`. Board inner row 21 (screen row 22) holds `▀` repeated 9 times at the paddle columns, and inner row 20 has a `▄` at the ball column.
- `dimmed_draw_has_no_key_chips`: with `dim: true` the text has no `←→` and no `␣`, and the HUD labels are still there.
- `brick_pixels_are_light_then_dark`: a board with a single `Brick(1)` at (0, 0). Screen cells (1, 3) to (3, 3) have fg 223 and cell (4, 3) has fg 180. A `Brick(5)` gives 124 and 88, and `Unbreakable` gives 245 and 239.
- `capsule_draws_a_letter_chip`: a W capsule at `(10.0, 20.0)` puts `W` with bg 45 at inner cell (10, 10).
- `paused_and_clear_draw_their_modals`: paused shows `PAUSED` and `stage 1  ·  0`. Phase `Clear { ticks: 60 }` at stage 1 with score 300 shows `STAGE 1 CLEAR`, `clear bonus`, `next life at stage 5` and `STAGE 2 IN 2`. Paused in that `Clear` phase shows `PAUSED` and `stage 1  ·  300`, which fails if the STAGE CLEAR modal is drawn last.

Verify with `cargo build`, `cargo test`, `cargo clippy --all-targets`. Manual play is not possible for the implementer; say so in the report.

Commit as `feat(brick-ball): game adapter, drawing, HUD, overlays and thumbnail`.

## Task 5: lock the frame to the pixel

Added after the operator's play test on 2026-10-02: at angles the ball looked jittery. Root cause: a fixed 33 ms tick moves the ball 0.5 to 0.9 pixels per axis per frame and `floor` sampling turns that into an irregular mix of no move, one pixel and diagonal jumps. The spec "Play" section now says the frame is locked to the pixel. This task implements that. It is one commit on top of Task 4 and includes the already edited `docs/spec.md` and this plan section.

Files: `games/brick-ball/src/board.rs`, `games/brick-ball/src/main.rs`, `games/brick-ball/src/draw.rs`, `docs/spec.md`, `docs/plan-brick-ball.md`.

### Rules, exact

- `pub const MIN_TICK: Duration = Duration::from_millis(16)` and `pub const IDLE_TICK: Duration = Duration::from_millis(33)`. `TICK`, `DT`, `ticks()`, `WIDE_TICKS`, `SLOW_TICKS`, `CLEAR_TICKS`, `STALL_TICKS` and `HOLD_RAMP` go away.
- Seconds constants: `WIDE_SECS: f64 = 15.0`, `SLOW_SECS: f64 = 10.0`, `STALL_SECS: f64 = 10.0`, `CLEAR_SECS: f64 = 2.0`, `HOLD_RAMP_SECS: f64 = 0.2`, `PADDLE_SPEED_MIN: f64 = 30.0` and `PADDLE_SPEED_MAX: f64 = 90.0` pixels per second.
- `pub fn tick_len(&self) -> Duration`: in `Play`, `1 / (ball_speed() × m)` seconds where `m` is the largest `max(|dx|, |dy|)` over the balls, never below `MIN_TICK`; in every other phase `IDLE_TICK`. `Board::tick(dir)` keeps its signature and advances the board by `dt = self.tick_len().as_secs_f64()` taken at the start of the tick, so the board moves by exactly the time the SDK waited.
- Timers are `f64` seconds: `wide`, `slow`, each ball's `stall`, and `Phase::Clear { left: f64 }`. They count down by `dt` only in the phases the spec names. `secs(left: f64) -> u32` is `left.ceil()` for the HUD and the clear modal, and the power bar fraction is `left / WIDE_SECS` or `left / SLOW_SECS`.
- Capsule: `cap.y += CAPSULE_SPEED * dt`. The catch rule is unchanged.
- Paddle: `hold: f64` is the seconds the current direction has been held, reset when the direction changes. Speed is `PADDLE_SPEED_MIN + (PADDLE_SPEED_MAX - PADDLE_SPEED_MIN) × min(hold / HOLD_RAMP_SECS, 1)`; the paddle moves `dir × speed × dt` and then `hold += dt`. The single press still moves exactly 1 pixel on the next tick. `paddle_step` is replaced by `pub fn paddle_speed(hold: f64) -> f64`.
- Ball: `dist = ball_speed() × dt`, sub-steps unchanged. For the fastest ball `dist × m` is one pixel up to floating point noise, so the drawn pixel is `(v + PIXEL_EPS).floor()` with `pub const PIXEL_EPS: f64 = 1e-6` in `draw.rs`, used for the ball and the capsule. Physics keeps plain `floor`. Plan decision.
- `main.rs`: `tick_rate()` returns `board::BLINK` while dying, `board::IDLE_TICK` while paused, else `self.board.tick_len()`.

### Tests, replacing the tick based ones

board.rs:

- `tick_len_is_one_major_pixel`: a single ball with `(dx, dy) = (0.6, 0.8)` at stage 1 speed gives `tick_len` within 1 µs of `1 / (28 × 0.8)` seconds; straight up at the cap speed gives `1 / 47.6`; a speed forced to 1000 gives `MIN_TICK`.
- `fastest_ball_sets_the_tick`: two balls, `(0.6, 0.8)` and `(0.98, 0.2)`; the tick follows `0.98`.
- `idle_phases_tick_at_33_ms`: `Serve`, `Clear` and `Over` return `IDLE_TICK`.
- `frame_moves_the_major_axis_exactly_one_pixel`: an empty grid, no bricks, one ball at `(30.3, 25.7)` for each of the angles 10°, 30°, 45° and 60° from vertical, 200 ticks each. On every tick where no wall was hit, `(major + PIXEL_EPS).floor()` changes by exactly 1 and the minor by 0 or 1. The test fails on the old fixed tick.
- `paddle_speed_ramps_over_a_fifth_of_a_second`: `paddle_speed(0.0) == 30.0`, `paddle_speed(0.1) == 60.0`, `paddle_speed(0.2) == 90.0`, `paddle_speed(1.0) == 90.0`. Holding Right from rest for one 33 ms tick moves the paddle by about 1 pixel.
- `timers_count_seconds_not_ticks`: catch W, then tick until the summed `dt` passes 15 s; `wide` is 0 only after that, whatever the tick length. The same for S and 10 s with a different ball angle, so the tick length differs.
- `capsule_falls_twenty_pixels_per_second`: over ticks summing to 1 s the capsule drops 20 px, at two different ball angles.
- `clear_counts_down_two_seconds`: `Clear { left: 2.0 }` reaches `next_stage` after ticks summing to 2 s.
- `stall_turns_after_ten_seconds`: adapted from the existing stall test to seconds.
- Every existing test that counted ticks is rewritten in seconds; keep its name and intent.

main.rs: `tick_rate_follows_the_ball`: in `Play` it equals `board.tick_len()`, paused it is `IDLE_TICK`, dying it is `BLINK`.

draw.rs: `power_bar_and_countdown_use_seconds`: `wide = 9.4` draws 4 of 5 bar marks and `10s`; `Clear { left: 1.2 }` shows `STAGE 2 IN 2`.

Verify with `cargo build`, `cargo test`, `cargo clippy --all-targets`. Manual play is not possible for the implementer; say so in the report.

Commit as `feat(brick-ball): lock the frame to the pixel so angled balls move evenly`.

## Task 6: fixed deadline, synchronized output, steady speed per tick, capsule as pixels

Added after play test 2 on 2026-10-06. The ball was still uneven while falling. Research (`.superpowers/sdd/plan-brick-ball/smooth-motion-research.md`) found three code defects, all fixed here, and a display refresh beat that Task 7 addresses. One commit on top of Task 5, including the already edited `docs/spec.md` and this plan section.

Files: `crates/sdk/src/lib.rs`, `games/brick-ball/src/board.rs`, `games/brick-ball/src/draw.rs`, `docs/spec.md`, `docs/plan-brick-ball.md`.

### Rules, exact

- SDK `run_loop` in `crates/sdk/src/lib.rs`: keep a fixed deadline. After `on_timer`, `next = next + app.tick_rate()`; if that is already at or before `now`, `next = now + app.tick_rate()` instead, so the loop never runs two ticks without a draw between them. When a key starts a game (`!was_playing && app.playing()`) the deadline restarts from now as today. Wrap each draw: `execute!(stdout, BeginSynchronizedUpdate)` before `terminal.draw` and `execute!(stdout, EndSynchronizedUpdate)` after it, from `crossterm::terminal` (crossterm 0.29 is already a dependency). Snake shares the loop and gets the same period.
- `Board::tick` reads `ball_speed()` once at the start, the same value `tick_len` used, and passes it to `move_ball`; the Slow timer and an S catch in the same tick change the speed from the next tick on. Plan decision: `move_ball(&mut self, i: usize, speed: f64)`.
- `draw.rs`: the capsule is drawn in `pixels()` as two pixels, `(floor(x), floor(y))` and the pixel below it, in `power_color(kind)`, after the bricks and before the balls; `paint` no longer writes a letter into a cell; the `chip` helper stays for the HUD only. `PIXEL_EPS` still applies to the capsule position.

### Tests

- `crates/sdk/src/lib.rs` or `app.rs`: extract the deadline rule into `pub fn next_deadline(prev: Instant, now: Instant, rate: Duration) -> Instant` and test it: a late tick advances from the previous deadline; a deadline already passed restarts from now; two consecutive late ticks never yield a deadline before now.
- `board.rs` `slow_changes_speed_only_from_the_next_tick`: a ball straight down at stage 1, catch S on a tick; that tick still moves the ball exactly one drawn pixel; when the timer runs out the ball still moves exactly one pixel on that tick. Fails without the fix (one still frame at the start, one two pixel jump at the end).
- `draw.rs` `capsule_draws_two_colored_pixels`: a W capsule at `(10.0, 20.0)` colors pixels (10, 20) and (10, 21) with 45, so inner cell (10, 10) shows `▀` with fg 45 and bg 45; no `W` glyph appears on the board; the HUD still shows the `W` chip when wide is active. Replaces `capsule_draws_a_letter_chip`.

Verify with `cargo build`, `cargo test`, `cargo clippy --all-targets`. Commit as `fix(brick-ball): fixed deadline, synchronized frames, steady speed per tick and a pixel capsule`.

## Task 7: smooth render mode under evaluation

One commit on top of Task 6. The pixel-locked tick cannot be a whole number of display refreshes (the app cannot read the refresh rate and the speed curve needs 28 to 47.6 px/s), so each one pixel step stays on screen for 4 or 5 refreshes at 120 Hz, which reads as uneven motion. The cure used by games is to render at a divisor of the refresh and move objects by sub-pixel amounts. In a terminal the sub-pixel position is shown by shading. This task adds that as a mode behind `ARCADE_RENDER`, default `smooth`, so the operator can compare; the losing mode is removed afterwards.

Files: `games/brick-ball/src/board.rs`, `games/brick-ball/src/main.rs`, `games/brick-ball/src/draw.rs`.

### Rules, exact

- `pub enum Render { Smooth, Locked }` in `main.rs`, read once at start from `ARCADE_RENDER` (`locked` gives `Locked`, anything else `Smooth`), stored on `BrickBall` and passed to the board and the draw. Plan decision: `Board` gets a `pub render: Render` field set by `BrickBall::new`, so `tick_len` can branch; put the enum in `board.rs` so both can use it.
- `Board::tick_len`: in `Smooth`, `Play` returns `SMOOTH_TICK: Duration = Duration::from_micros(16_667)`; every other phase and `Locked` behave as today.
- `draw.rs` `pixels()` in `Smooth`: moving objects are drawn by coverage. A ball at `(x, y)` is a 1 by 1 square covering up to four pixels; each pixel gets coverage `cx × cy` where `cx = 1 - |x - px|` clamped to 0..1 and likewise for `cy`. The ball pixel color is the field color 233 blended toward 255 by coverage through the grey ramp: index `233 + round(coverage × 22)`, so coverage 1 gives 255 and the total light over the four pixels is constant. A pixel already holding a brick or the paddle keeps its color unless the ball coverage is at least 0.5, then it shows the ball shade (plan decision: no color blending over bricks). The capsule block is drawn the same way with its power color: coverage below 0.5 leaves the field, 0.5 and above paints the power color (plan decision: no blend for colored objects; the xterm cube is too coarse). The paddle keeps its current drawing. Several balls add their coverage per pixel, capped at 1. In `Locked`, drawing is unchanged (`PIXEL_EPS` floor).
- `main.rs` `tick_rate` is unchanged in shape; it returns `board.tick_len()` which now branches on the mode.

### Tests

- `board.rs` `smooth_mode_ticks_at_sixty_hertz`: in `Smooth`, `Play` gives `SMOOTH_TICK`; `Serve` gives `IDLE_TICK`; `Locked` still gives the pixel time.
- `draw.rs` `smooth_ball_shades_two_pixels_by_coverage`: `Smooth`, a ball at `(30.0, 20.5)` over the field lights pixels (30, 20) and (30, 21) with grey 244 each (`233 + round(0.5 × 22)`); at `(30.0, 20.0)` only pixel (30, 20) with 255; at `(30.5, 20.5)` four pixels with 239 (`233 + round(0.25 × 22) = 239`).
- `draw.rs` `locked_mode_draws_as_before`: the existing ball pixel test passes with `Locked`.
- `main.rs` `render_mode_comes_from_the_environment`: a helper `Render::from_env_value(Option<&str>)` maps `Some("locked")` to `Locked` and `None` or `Some("smooth")` to `Smooth`; `std::env` is read only in `main`.
- `frame_moves_the_major_axis_exactly_one_pixel` keeps passing in `Locked`; in `Smooth` the drawn pixel is not expected to change every frame, so that test sets `Locked`.

Verify with `cargo build`, `cargo test`, `cargo clippy --all-targets`. Commit as `feat(brick-ball): smooth render mode with sub-pixel shading behind ARCADE_RENDER`.

## Task 8: smooth render becomes the rule, with a squared falloff

Added after play test 3 on 2026-10-06. The operator compared the two render modes and chose smooth, and asked for less halo around the ball. The spec "Play" section is now final: fixed 16.67 ms tick, sub-pixel shading by `c²`, no render modes. One commit on top of Task 7, including the already edited `docs/spec.md` and this plan section.

Files: `games/brick-ball/src/board.rs`, `games/brick-ball/src/main.rs`, `games/brick-ball/src/draw.rs`, `docs/spec.md`, `docs/plan-brick-ball.md`.

### Rules, exact

- Remove the `Render` enum, `Render::from_env_value`, the `RENDER` `OnceLock`, the `ARCADE_RENDER` read in `main`, the `render` field on `Board`, `MIN_TICK`, and the pixel-locked branch of `tick_len`. `SMOOTH_TICK` is renamed `PLAY_TICK` (`Duration::from_micros(16_667)`); `tick_len` returns `PLAY_TICK` in `Play` and `IDLE_TICK` otherwise. Remove `PIXEL_EPS` if nothing uses it after the change.
- `draw.rs`: a `fn shade(c: f64) -> u8` returns `FIELD_GREY + round(c.clamp(0, 1)² × (BALL - FIELD_GREY))`, so `shade(1.0) == 255`, `shade(0.5) == 239`, `shade(0.25) == 234`, `shade(0.0) == 233`. Ball pixels use it; the coverage sum of several balls is clamped to 1 before shading. The brick and paddle rule (a lit pixel keeps its color unless coverage is at least 0.5) and the capsule threshold rule are unchanged.
- The `Serve` special case in `pixels()` goes: the resting ball is drawn by the same centered rule. `rest_ball` puts the ball at `y = PADDLE_Y - 0.5` (41.5), so its centered square is exactly pixel row 41, and `x = ceil(paddle_cx) - 0.5`, the center of the paddle's middle pixel, so the ball is one full pixel also when `paddle_cx` is on a pixel edge (it starts at 30.0).
- The capsule needs no clock: at 20 px/s and 60 ticks per second it moves exactly a third of a pixel per tick and the half threshold makes it step one pixel every third tick.

### Tests

- Delete the tests that only hold for the locked mode: `frame_moves_the_major_axis_exactly_one_pixel`, `tick_len_is_one_major_pixel`, `fastest_ball_sets_the_tick`, `smooth_mode_ticks_at_sixty_hertz` (replaced below), `locked_mode_draws_as_before`, `render_mode_comes_from_the_environment`.
- `board.rs` `play_ticks_at_sixty_hertz`: `Play` gives `PLAY_TICK`, `Serve`, `Clear` and `Over` give `IDLE_TICK`.
- `board.rs`: the tests that were forced into locked mode (`a_hit_puts_the_ball_back_outside_the_brick`, `capsule_that_passed_the_paddle_row_beside_the_paddle_is_not_caught`, `ball_that_passed_the_paddle_row_beside_the_paddle_is_not_bounced`, `slow_changes_speed_only_from_the_next_tick`, the timer and capsule tests) are rewritten for the fixed tick, keeping their names and intent; positions are chosen for a step of `speed × 1/60` pixels. `slow_changes_speed_only_from_the_next_tick` now asserts that the move on the catch tick uses the pre-catch speed and the move on the following tick uses the slowed speed.
- `board.rs` `capsule_steps_one_pixel_every_third_tick`: over 30 play ticks the capsule's drawn row (the row whose coverage is at least half) changes exactly every third tick.
- `board.rs` `rest_ball_sits_on_pixel_row_41`: `rest_ball().y == 41.5`.
- `draw.rs` `shade_is_squared`: the four values above. `smooth_ball_shades_two_pixels_by_coverage` is updated to the squared values: a ball centered at `(30.5, 21.0)` lights (30, 20) and (30, 21) with 239 each; centered at `(30.5, 20.5)` only (30, 20) with 255; centered at `(31.0, 21.0)` four pixels with 234.
- `draw.rs` `serve_ball_is_drawn_like_a_moving_ball`: a board in `Serve` with `paddle_cx` 30.0, 30.2, 30.5 and 30.7 lights exactly pixel (29, 41), (30, 41), (30, 41) and (30, 41) with 255, the paddle's middle pixel, and nothing in rows 40 or 42.

Verify with `cargo build`, `cargo test`, `cargo clippy --all-targets`; `grep -rn "Render\|ARCADE_RENDER\|Locked\|MIN_TICK\|SMOOTH_TICK" games/brick-ball/src` must find nothing. Commit as `feat(brick-ball): smooth render becomes the rule with a squared falloff`.

## Task 9: bright core shading and hold on press

Added after play test 4 on 2026-10-06. The operator still saw a halo with the `c²` falloff, and found the paddle slow to respond in Terminal.app. The spec "Held keys" paragraph (SDK) and the shading and paddle sentences in "Brick ball" > "Play" are already updated. One commit on top of Task 8, including the edited `docs/spec.md` and this plan section.

Files: `crates/sdk/src/input.rs`, `crates/sdk/src/game.rs`, `crates/sdk/src/app.rs`, `crates/sdk/src/testkit.rs`, `crates/sdk/src/lib.rs` (where `Input::new` is called), `games/snake/src/main.rs` (`hold_on_press: false`), `games/brick-ball/src/main.rs`, `games/brick-ball/src/board.rs`, `games/brick-ball/src/draw.rs`, `docs/adding-a-game.md` (the new `Entry` field), `docs/spec.md`, `docs/plan-brick-ball.md`.

### Rules, exact

SDK `Input`, terminals without release events:

- Constants: `REPEAT_WINDOW` stays 150 ms and becomes the cap; `MIN_WINDOW: Duration = 30 ms`; `FIRST_REPEAT: Duration = 600 ms`; `PRESS_HOLD: Duration = 300 ms`; `REPEAT_FACTOR: f64 = 1.5`.
- `Input::new(release_events: bool, hold_on_press: bool)`. New state: `repeats: u32` (follow-up presses of the current key) and `interval: Option<Duration>` (the gap between the last two repeats).
- `press(key, now)`: a press of the same key as `last_press` within `gap_limit` continues the hold, where `gap_limit` is `FIRST_REPEAT` while `repeats == 0` with `hold_on_press`, and the current window otherwise, so 150 ms for the first repeat without `hold_on_press` and Snake plays as before (fix round 4). On a continuing press: `repeats += 1`; when `repeats >= 2`, `interval = Some(now - last_press_time)`; `key = Some(key)`; `until = Some(now + window)`. The window is `REPEAT_WINDOW` while `interval` is `None`, otherwise `(interval × REPEAT_FACTOR).clamp(MIN_WINDOW, REPEAT_WINDOW)`. On a new press (a different key, or the same key after the limit): `repeats = 0`, `interval = None`; with `hold_on_press`, `key = Some(key)` and `until = Some(now + PRESS_HOLD)`; without it, `key = None`. `last_press = Some((key, now))` in both cases. `held`, `clear`, `set_now` and the release-events path are unchanged.
- `Entry` gains `pub hold_on_press: bool` after `stages`, documented: "Without release events a press starts a hold for 300 ms. For continuous controls such as a paddle. Leave false for tap controls." `App` or `lib::run` passes it to `Input::new`. Snake: `hold_on_press: false`. testkit fakes: `false`. `docs/adding-a-game.md` lists the field.

Brick ball:

- `ENTRY.hold_on_press: true`. Remove the single press rule: `Board::press` and its `pressed` state go, `BrickBall::key` no longer handles Left, Right, `a`, `d`; the tests `single_press_moves_one_pixel_on_the_next_tick` and `tap_moves_the_paddle_one_pixel` go.
- `board.rs` (fix round 4): `HOLD_GAP_SECS: f64 = 0.3` and a `still` field, seconds since the paddle last moved. In `move_paddle`, `dir == 0` adds `dt` to `still` and resets `hold` and `hold_dir` only when `hold < HOLD_RAMP_SECS` or `still > HOLD_GAP_SECS`; a nonzero `dir` sets `still = 0`. This covers the stop between the 300 ms press hold and a first OS repeat up to 600 ms after the press.
- `draw.rs`: for each ball, the overlapped pixel with the largest coverage (ties: the first in row-major order) is drawn at `BALL` 255. Every other overlapped pixel gets `trace(c) = FIELD_GREY + round(c³ × 22)`, at most 236. Traces of several balls add before the cube; a core always wins over a trace. The brick and paddle rule is unchanged: a lit pixel keeps its color unless the ball covers at least half of it, and a core pixel always covers at least a quarter, so a core over a brick shows only when its coverage is at least half. `shade` is replaced by `trace`. Temporary comparison switch, removed after the operator decides: environment variable `ARCADE_SHADE=squared` restores the Task 8 rule (no fixed core, `233 + round(c² × 22)`); read once in `main` as in Task 7, default is the new rule. Plan decision.

### Tests

- `input.rs`: `first_repeat_within_600_ms_continues_the_hold` (with `hold_on_press`, press at 0, repeat at 375 ms: held at 380, not at 525, one window after the repeat); `window_follows_the_measured_interval` (with `hold_on_press`, repeats 90 ms apart: held 100 ms after the last, not 150; repeats 15 ms apart: window is 30 ms); `hold_on_press_starts_at_the_press_and_ends_after_300_ms_without_repeats` (held at 10 ms and 290 ms, not at 310); `hold_on_press_hands_over_to_repeats` (press at 0, repeats at 375 and 465: held at 500, not held at 465 + 150 = 615 + 1); `without_hold_on_press_a_tap_is_not_a_hold` (the existing first assertion, kept, and a second press 375 ms later is not a hold either); `release_events_hold_from_press_to_release` unchanged; `key_repeat_holds_until_repeats_stop` adapted to the new windows, the hold starting at the second repeat.
- `app.rs`: an existing hold test passes `hold_on_press` through; one assertion that `Input::new(false, entry.hold_on_press)` is what `App` builds for a `FAKE` with `hold_on_press: true` (held right after a press).
- `board.rs`: remove the press tests; `paddle_speed_ramps_over_a_fifth_of_a_second` unchanged. Fix round 4: `paddle_keeps_full_speed_through_a_short_stop` (a stop below full speed restarts the ramp at 0.99 px per 33 ms tick, a 66 ms stop at full speed keeps 2.97); `paddle_accelerates_from_one_to_three_and_stops_at_walls` stops for 0.33 s before checking the restart.
- `main.rs`: `keys_are_ignored_while_dying` keeps only the `p` and space checks.
- `draw.rs`: `ball_has_one_full_core_and_faint_traces`: a ball centered at `(30.5, 21.0)` covers (30, 20) and (30, 21) by 0.5 each; the first in row-major order, (30, 20), is 255 and (30, 21) is 236 (`233 + round(0.125 × 22) = 236`); centered at `(31.0, 21.0)` four pixels at 0.25: (30, 20) is 255 and the other three are 233 (`round(0.015625 × 22) = 0`); centered at `(30.75, 21.0)`: (30, 20) with 0.75 is 255, (30, 21) with 0.25 is 233. `two_balls_each_have_a_core`. `squared_shade_is_only_behind_the_switch`: with the switch value the Task 8 values hold (239 at 0.5), without it the new ones.

Verify with `cargo build`, `cargo test`, `cargo clippy --all-targets`. Commit as `feat(sdk): hold on press without release events, brick ball ball core shading`.

## Task 10: squared shading stays, gentle paddle until a hold is confirmed

Added after play test 5 on 2026-10-06. The operator chose the Task 8 shading over the bright core (the core stepped visibly) and found the Terminal.app paddle too sensitive (a 300 ms press-hold under the full ramp moved about 21 px per tap). The spec is already updated. Two commits on top of Task 9, one per change, the spec and plan edits going with the second.

### Commit A: drop the bright core shading

Files: `games/brick-ball/src/draw.rs`, `games/brick-ball/src/main.rs`.

- Remove the core and trace rule, the `ARCADE_SHADE` switch (its `OnceLock`, the read in `main`, the static in `draw.rs`) and `trace`. Restore the Task 8 rule exactly: `shade(c) = FIELD_GREY + round(c.clamp(0, 1)² × (BALL - FIELD_GREY))` for every overlapped pixel, several balls summed and clamped to 1 before shading, the brick and paddle half rule unchanged.
- Tests: `shade_is_squared` (255, 239, 234, 233) and `smooth_ball_shades_two_pixels_by_coverage` back to the Task 8 values (a ball centered at `(30.5, 21.0)` lights (30, 20) and (30, 21) with 239; at `(30.5, 20.5)` only (30, 20) with 255; at `(31.0, 21.0)` four pixels with 234). Remove `ball_has_one_full_core_and_faint_traces`, `two_balls_each_have_a_core`, `squared_shade_is_only_behind_the_switch`; keep a two-ball test that checks coverage adds (two balls each covering a pixel by 0.5 light it with 255).
- Commit as `fix(brick-ball): drop the bright core shading, the squared falloff stays`.

### Commit B: provisional holds move the paddle gently

Files: `crates/sdk/src/input.rs`, `games/brick-ball/src/board.rs`, `games/brick-ball/src/main.rs`, `docs/adding-a-game.md` (one sentence on `Input::provisional` next to `held`), `docs/spec.md`, `docs/plan-brick-ball.md`.

- `input.rs`: `PRESS_HOLD` becomes 400 ms. New `pub fn provisional(&self, key: KeyCode) -> bool`: true when `held(key)` and the hold rests on the press alone, that is no release events, `hold_on_press`, and `repeats == 0`. False as soon as a repeat continues the hold, and always false with release events.
- `board.rs`: remove `HOLD_GAP_SECS`, the `still` field and the keep-alive branch in `move_paddle`. `move_paddle(&mut self, dir: i32, dt: f64, provisional: bool)`: with `dir == 0` reset `hold` to 0 as before Task 9; when `provisional`, move by `PADDLE_SPEED_MIN × dt` and leave `hold` at 0; otherwise ramp as before. `Board::tick(dir, provisional)` passes it through; `Serve` and `Play` both move the paddle.
- `main.rs`: the adapter computes `dir` as today and `provisional` as `input.provisional(key)` for the key that gave `dir` (Left or `a`, Right or `d`).
- Tests: `input.rs` `press_hold_is_provisional_until_a_repeat` (held and provisional at 10 ms, not provisional after the first repeat at 375 ms, not held at 401 ms with no repeat) and `release_events_are_never_provisional`. `board.rs` `provisional_hold_moves_at_the_start_speed` (0.4 s of provisional ticks moves 12 px and leaves `hold` at 0), `ramp_starts_when_the_hold_is_confirmed` (after 0.4 s provisional, confirmed ticks accelerate from 30 to 90 over 0.2 s), `paddle_accelerates_from_one_to_three_and_stops_at_walls` back to its Task 5 form with the stop resetting the ramp; remove `paddle_keeps_full_speed_through_a_short_stop`. `main.rs`: one test that a provisional Right hold moves the paddle `PADDLE_SPEED_MIN × dt` on a tick.
- Commit as `feat(sdk): provisional holds so a paddle moves gently until a repeat confirms the key`.

Verify with `cargo build`, `cargo test`, `cargo clippy --all-targets`; `grep -rn "ARCADE_SHADE\|trace(\|HOLD_GAP" games/brick-ball/src` must find nothing.
