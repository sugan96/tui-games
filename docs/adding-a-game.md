# Adding a game

A game is one binary crate under `games/` that links `arcade-sdk`. The SDK draws the game menu, the top 10, name entry and game over, and keeps the scores. You write the rules, the keys and the drawing.

## 1. Create the crate

```
games/pong/Cargo.toml
games/pong/src/main.rs
```

```toml
[package]
name = "arcade-pong"
version.workspace = true
edition.workspace = true

[dependencies]
anyhow.workspace = true
arcade-sdk.workspace = true
ratatui.workspace = true
```

Add `"games/pong"` to `members` in the root `Cargo.toml`.

## 2. Describe it

```rust
use arcade_sdk::{DrawCtx, Entry, Game, Input, Outcome, Status};

const ENTRY: Entry = Entry {
    id: "pong",                  // score key and install name, never change it after release
    title: "pong",               // block letter logo, letters and spaces
    about: "first to 7 wins",    // one line on the game menu
    starts: &[("1-3", "level")], // key chips on the game menu
    stages: false,               // true shows a stage picker on the menu and passes the chosen stage
    start,                       // fn(char, u32) -> Option<Box<dyn Game>>
    min_size: (80, 24),          // smallest terminal your draw fits in
    thumb: &[                    // library card art: 12 rows of 24 palette letters
        "........................",
        // ... 12 rows in all, see "Thumbnails" in docs/spec.md for the palette
    ],
};

fn main() -> anyhow::Result<()> {
    arcade_sdk::run(&ENTRY)
}
```

`start` gets every key pressed on the game menu except `q`, Enter as `'\n'`, and the chosen stage, or 1 without stages. Return `None` for keys you do not use.

## 3. Implement `Game`

```rust
impl Game for Pong {
    fn key(&mut self, key: KeyCode) { /* presses only */ }
    fn tick(&mut self, input: &Input) -> Status { /* Running, or Over(Outcome) once */ }
    fn tick_rate(&self) -> Duration { /* time to the next tick, asked after every tick */ }
    fn draw(&self, frame: &mut Frame, ctx: &DrawCtx) { /* ctx.dim under modals, ctx.best for a HUD */ }
}
```

- `q` quits on every screen and never reaches the game. Do not use it as a game key.
- For continuous movement, check `input.held(KeyCode::Left)` in `tick`. Key repeats and releases reach `Input`, not `key`.
- Play any death animation yourself, then return `Status::Over(Outcome { score, variant, summary })` once. `variant` shows in the top table, for example "level 2". `summary` is one line on the game over screen.
- A game with stages implements `fn reached(&self) -> u32`, the highest stage the run has got to. The SDK saves it as progress when the run ends and when the player quits with `q` during a run. A game without stages leaves it out and gets the default 0.
- When `ctx.dim` is true, draw muted and leave out key hints. A modal covers the middle.
- Use `arcade_sdk::ui::{theme, font, panel, centered}` so the game matches the rest of the arcade.

`games/brick-ball` is the reference for a game with stages and `games/snake` for one without. Both keep the rules in `board.rs` with no terminal code, the `Game` adapter in `main.rs` and the drawing in `draw.rs`.

## 4. Test it

```rust
#[test]
fn meets_the_arcade_contract() {
    arcade_sdk::testkit::check(&ENTRY);
}
```

`check` also fails unless the thumbnail is exactly 12 rows of 24 palette letters. It starts every start key, feeds a stream of keys, draws at `min_size` live and dimmed, and fails if a game never ends, returns a zero tick rate, or has a bad id or title. It also starts a game with stages at stage 20 and checks that `reached` is at least the start stage. `testkit::render` returns a drawn frame as text for your own drawing tests.

## 5. Play it

```sh
cargo run -p arcade-pong                                         # on its own
cargo build -p arcade-pong
cargo run -p arcade -- publish target/debug/arcade-pong          # into the local catalog, shows as GET
cargo run -p arcade                                              # the library: Enter gets, Enter again plays
cargo run -p arcade -- install target/debug/arcade-pong          # or skip the catalog and install directly
```
