# Arcade spec

Snake rules confirmed with the operator on 2026-09-24, look on 2026-09-29.
The multi-game layout and the library confirmed on 2026-10-01. This is the binding authority.

## Purpose

A collection of terminal games in one tool. More games arrive over time, and contributors write them.
Each game is its own executable, so the player installs only the games they want and removes them to free the space.
Later the launcher lists games from a cloud catalog and downloads them. For now games install from a local file.

## Parts

- `arcade` (crates/launcher): the game library. Shows installed and catalog games, plays, gets and removes them.
- `arcade-sdk` (crates/sdk): the library every game links. It holds the `Game` and `Entry` contract, the game menu with its top 10, name entry, game over, the score table, held-key tracking, the theme, the block font, and `testkit::check`.
- One binary crate per game under `games/`. Its main is `arcade_sdk::run(&ENTRY)`. It runs on its own too.

## Library (launcher)

Confirmed with the operator on 2026-10-01.

`arcade` opens the library: one grid of cards holding installed games and games in the catalog that are not installed yet. Each card has the game's thumbnail, its title, and a status line: `▶ played 2h ago`, `● installed`, `⇩ GET  2.9 MB`, or `✖ broken`. The selected card has a thick accent border. The selected game's about line and the last message sit under the grid. The grid wraps to the terminal width and scrolls by rows to keep the selection in view.

Order: installed games that were played, most recent first. Then installed games never played. Then catalog games not installed. A to Z inside the last two tiers and among ties.

Keys: arrows or `hjkl` move. Enter plays an installed game or gets a catalog game. `x` then `x` again removes an installed game, and any other key cancels. `q` or Esc quits.
While a game runs, the library gives it the terminal and waits. When the game exits, the library comes back, with a red line if the game failed.

Folders, under `~/.local/share/arcade/`: `games/` for installed games, `catalog/` for the local catalog (or the folder named by `ARCADE_CATALOG`), and `last-played` with one `<id> <unix secs>` line per game, written through a temp file and a rename.
Both game folders hold `<id>`, the executable, and `<id>.info`, its info text. Listing reads only info files and never runs a binary. A binary without an info file, an info file without its binary, or a bad info file shows as broken.

The local catalog stands in for the cloud catalog. Get copies the binary and its info into `games/` without running it. The cloud catalog will serve the same two things per game, an info text and a binary.

CLI: `arcade install <file>` runs `<file> --info` once and adds the game to `games/`. `arcade publish <file>` does the same into the catalog. `arcade remove <id>` deletes a game and its info. `arcade list` prints every game with `installed`, `available` or the broken reason.

## Game info protocol

`<game> --info` prints `key=value` lines and exits without touching the terminal or the score file:
`protocol=1`, `id`, `title`, `about`, and one `thumb=` line per thumbnail row. Unknown keys are ignored. Another protocol number is refused.
The id is lower case letters and dashes and is the file name in the games folder, so a game cannot write outside it.
The launcher kills a game that takes longer than 2 seconds to answer. It only asks on install and publish.

## Thumbnails

A thumbnail is 12 rows of 24 palette letters, one letter per pixel, `.` for transparent. Each terminal cell draws two stacked pixels with `▀`, foreground on top and background below, so a thumbnail fills 24 columns by 6 rows and pixels look square. Palette, xterm 256: `k` 16 black, `w` 255 white, `a` 244 grey, `d` 238 dark grey, `r` 196 red, `R` 124 dark red, `o` 208 orange, `y` 226 yellow, `Y` 220 gold, `l` 118 lime, `g` 40 green, `G` 28 dark green, `c` 51 cyan, `b` 33 blue, `B` 19 navy, `p` 93 purple, `m` 201 magenta, `n` 130 brown, `s` 173 terracotta, `S` 209 light terracotta. A game without a thumbnail shows its initial in block letters.

## Game menu, name entry, game over (SDK)

The game menu shows the title as a block letter logo, the top 10 of that game, a blinking `PRESS <keys> TO START` line built from the start keys, and the game's one line about text. Key chips sit in the bottom border.
`q` quits the game on every screen except name entry, and the player lands back in the launcher.
Game over shows the score in block digits, a `new best` tag when it made the table, the game's summary line and its variant. `r` returns to the game menu.
If the score qualifies, name entry comes first: type `A` to `Z` (any case, stored upper case), Backspace removes the last letter, Enter confirms only with exactly 3 letters.
Both are modals over the game, which draws itself dimmed underneath.

Held keys: where the terminal supports the kitty keyboard protocol, release events are on and a key is held from press to release. Elsewhere a second press of the same key within 150ms of the previous one is a hold, and the hold ends 150ms after the last repeat. One key is held at a time, and the last press wins.

## Storage

SQLite via rusqlite with the bundled feature, at `~/.config/arcade/scores.db`, shared by every game.
One table `scores`: `id INTEGER PRIMARY KEY, game TEXT NOT NULL, variant TEXT NOT NULL, name TEXT (nullable), score INTEGER NOT NULL, played_at TEXT NOT NULL` where `played_at` defaults to UTC ISO 8601 from SQLite.
Every finished game gets a row with name NULL. When the player enters a name, the row is updated.
The top 10 of a game is a query over its named rows, ordered by score descending then id ascending.
A score qualifies when it is above 0 and either fewer than 10 named rows of that game exist or it is strictly greater than the 10th.

On first use, if `scores.db` does not exist and `~/.config/snake/snake.db` does, it is copied over and left in place. Opening a file with the snake-only `games` table moves its rows into `scores` as game `snake`, with mode 0 as `variable` and 1 to 9 as `fixed N`. `PRAGMA user_version` 1 marks the new schema.
If the database file exists but is not a valid SQLite database, it is renamed to `<file>.broken-<unix seconds>` and a fresh one is created. Never deleted.
If the config directory cannot be created or written, the game exits with an error before entering the terminal UI.

## Look

Colors are xterm 256 indexes only, so every terminal shows the same thing. Terracotta 173 and 209 for accents and score digits, amber 214 for HUD labels, gold, silver and bronze for the top three ranks, greys 233 to 254 for chrome and text. Glyphs are Unicode block elements and geometric shapes, no icon font. Panels are double-line. Every screen title is a block letter logo with a drop shadow over a marquee stripe.

## Snake

Arrow keys steer. Reversing direction is ignored, even across several turns in one step. Holding the arrow for the current direction doubles the speed until it is released.
Start keys: `1` to `9` for that fixed level, `v` for variable. Variant labels are `fixed N` and `variable`.

Grid is 40 by 20. Step time in milliseconds is `220 - level * 20`, so level 1 is 200ms and level 9 is 40ms.
Fixed mode keeps its level. Variable mode starts at level 1 and rises one level every 3 apples, capped at 9.
Each apple is worth the level at the moment it is eaten. A score of 0 never qualifies. The summary line is the apples eaten.

Board on the left with a checkerboard field, HUD on the right with 1UP score in block digits, HI-SCORE from the top table, level meter out of 9, apples, mode and speed. Phosphor green 118 to 28 for the snake fading head to tail, yellow 226 head, red 196 apple. The snake is solid blocks with a pointed head that faces its direction of travel. Food is a two-cell pixel apple that blinks every step.

Motion: the game ticks at half the step time. After a step the renderer draws the head half a cell ahead of the neck and the tail half a cell out of its old cell, then the half tick settles the frame on the cells. Horizontal halves are one column, vertical halves are half blocks.

Death: on collision the snake blinks rust for six half-periods of 200ms, with steering ignored, then the game is over.

Minimum terminal is 107 by 28.

## Out of scope

The cloud catalog, updating an installed game from a newer catalog copy, pause, sound, stats screen, signing game binaries, running games in a sandbox.
