# Snake arcade spec

Confirmed with the operator on 2026-09-24. This is the binding authority. The plan argues from it.

## Purpose

A terminal snake game the operator plays, built to grow into something bigger.

## Screens and keys

Start screen (menu): shows the top 10 table and a speed picker.
- `1` to `9`: start a game at that fixed level.
- `v`: start a game in variable mode.
- `q`: quit.
- One line of help text explains scoring: an apple is worth the current level in points.

Playing: arrow keys steer. Reversing direction is ignored. `q` quits the program.
The border title shows mode, level, apples, and score.

Game over: shows final score. `r` returns to the menu. `q` quits.
If the score qualifies for the top 10, the name entry screen comes first, then game over.

Name entry: type `A` to `Z` (any case, stored upper case), Backspace removes the last letter,
Enter confirms. Enter is ignored unless exactly 3 letters are entered. Letters beyond 3 are ignored.

## Speed

Grid is 40 by 20. Tick duration in milliseconds is `220 - level * 20`, so level 1 is 200ms and level 9 is 40ms.
Fixed mode keeps its level. Variable mode starts at level 1 and rises one level every 3 apples, capped at 9.

## Scoring

Each apple is worth the level at the moment it is eaten. Score is the running total. Apples eaten is tracked separately.
A score of 0 never qualifies for the table.

## Storage

SQLite via rusqlite with the bundled feature. File at `~/.config/snake/snake.db`.
One table `games`: `id INTEGER PRIMARY KEY, name TEXT (nullable), score INTEGER NOT NULL, apples INTEGER NOT NULL, mode INTEGER NOT NULL, played_at TEXT NOT NULL` where `played_at` defaults to UTC ISO 8601 from SQLite.
Mode is stored as an integer: 0 for variable, 1 to 9 for fixed.
Every finished game gets a row, name NULL. When the player enters a name, the row is updated.
Top 10 is a query over rows with a name, ordered by score descending then id ascending.
A score qualifies when it is above 0 and either fewer than 10 named rows exist or it is strictly greater than the 10th named score.

If the database file exists but cannot be opened as a SQLite database, rename it to `snake.db.broken-<unix seconds>` and create a fresh one. Never delete it.
If the config directory cannot be created or written, exit with an error before entering the terminal UI.

## Layout

`src/main.rs` terminal loop only. `src/app.rs` screen state and input handling. `src/game.rs` grid, snake, speed, score, no terminal or database imports. `src/db.rs` all SQLite. `src/ui/mod.rs` dispatch, `src/ui/menu.rs`, `src/ui/play.rs`, `src/ui/game_over.rs`, `src/ui/name_entry.rs` one file per screen. `anyhow` for errors.

## Out of scope

Pause, sound, stats screen, arrow key name entry, score persistence anywhere but SQLite.
