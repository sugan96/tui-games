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
Before either, the dead snake blinks for about 1.2 seconds.

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

## Look

Confirmed with the operator on 2026-09-29 from a colored mockup. Arcade direction.

Colors are xterm 256 indexes only, so every terminal shows the same thing. Terracotta 173 and 209 for accents and score digits, amber 214 for HUD labels, phosphor green 118 to 28 for the snake fading head to tail, yellow 226 head, red 196 apple, gold, silver and bronze for the top three ranks, greys 233 to 254 for chrome and text. Glyphs are Unicode block elements and geometric shapes, no icon font.

Menu: one double-line panel. Block letter SNAKE logo with a drop shadow, marquee stripe under it, top 10 with star marks on the podium and dot placeholders for empty rows, a blinking PRESS 1-9 OR V TO START line, and a speed picker with one chip per level over a bar ramp. Key hints sit as chips in the bottom border.

Playing: board on the left in a double-line panel with a checkerboard field, HUD panel on the right with 1UP score in block digits, HI-SCORE from the top table, level meter out of 9, apples, mode and tick. The snake is solid blocks with a pointed head that faces its direction of travel. Food is a two-cell pixel apple that blinks every tick.

Motion: the timer fires at half the tick. After a step the renderer draws the head half a cell ahead of the neck and the tail half a cell out of its old cell, then the half tick settles the frame on the cells. Horizontal halves are one column, vertical halves are half blocks.

Death: on collision the snake blinks rust six half-periods of 200ms with keys ignored except `q`, then the game is recorded and the modal appears.

Name entry and game over are modals over the frozen, dimmed board. Game over shows the score in block digits and a `new best` tag when the score made the table. Name entry shows the score, the rank it will take, and three letter boxes.

Minimum terminal is 108 by 28.

## Layout

`src/main.rs` terminal loop only. `src/app.rs` screen state and input handling. `src/game.rs` grid, snake, speed, score, no terminal or database imports. `src/db.rs` all SQLite. `src/ui/mod.rs` dispatch, `src/ui/menu.rs`, `src/ui/play.rs`, `src/ui/game_over.rs`, `src/ui/name_entry.rs` one file per screen. `anyhow` for errors.

## Out of scope

Pause, sound, stats screen, arrow key name entry, score persistence anywhere but SQLite.
