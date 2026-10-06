# Arcade spec

Snake rules confirmed with the operator on 2026-09-24, look on 2026-09-29.
The multi-game layout and the library confirmed on 2026-10-01. Brick ball and stage progress confirmed on 2026-10-02. This is the binding authority.

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

Stage progress, for games with `Entry::stages` true: the menu shows `◀ STAGE n ▶ of m` on the line above the start prompt, where `m` is the highest stage the player has reached in that game (at least 1) and `n` is 1 when the game opens and keeps its value between runs. `←` and `→` move `n` within 1 to `m`; an arrow that cannot move is drawn dim. `start` is `fn(char, u32)` and gets the chosen stage, or 1 for a game without stages. Enter reaches `start` as `'\n'`, so a game can list `enter` as a start key. `Game::reached(&self) -> u32` is the highest stage the run has got to, with a default of 0 so games without stages leave it out. The SDK raises the saved progress to it when a run ends and when the player quits with `q` during a run, so a long run quit early keeps its unlocks. A game without stages shows no picker and gets stage 1.

Tick scheduling: the SDK keeps a fixed deadline. After a tick the next deadline is the previous deadline plus the game's tick rate, so poll lateness and draw time do not accumulate and the game clock keeps real time; when that deadline has already passed the loop restarts from now and never runs catch-up ticks back to back. Every frame is wrapped in synchronized output (DEC mode 2026) so terminals that support it show the frame whole; others ignore it.

Held keys: where the terminal supports the kitty keyboard protocol, release events are on and a key is held from press to release. Elsewhere the OS key repeat is the signal: a press of the same key while its hold is still open continues it, and the hold stays open for 1.5 times the repeat gap after the last repeat, where the gap is the larger of the last two gaps measured between repeats so one late event read cannot break a hold, at least 30 ms and at most 150 ms, or 150 ms until a gap has been measured. A game may set `Entry::hold_on_press`; then, without release events, a press starts a hold at once for up to 400 ms, which covers the 375 ms macOS default repeat delay, and a first repeat within 600 ms of the press continues that hold. Such a hold is provisional until a repeat confirms it, and `Input::provisional` tells a game so, so it can move gently until then; the price is that a short tap counts as a 400 ms hold. Without `hold_on_press` the first repeat after the OS delay does not count, so a hold starts at the second repeat, two taps of a key are not a hold, and Snake plays as before. One key is held at a time, and the last press wins. Confirmed on 2026-10-06 after play in Terminal.app, where the paddle waited for the OS repeat delay and coasted after release.

## Storage

SQLite via rusqlite with the bundled feature, at `~/.config/arcade/scores.db`, shared by every game.
One table `scores`: `id INTEGER PRIMARY KEY, game TEXT NOT NULL, variant TEXT NOT NULL, name TEXT (nullable), score INTEGER NOT NULL, played_at TEXT NOT NULL` where `played_at` defaults to UTC ISO 8601 from SQLite.
Every finished game gets a row with name NULL. When the player enters a name, the row is updated.
The top 10 of a game is a query over its named rows, ordered by score descending then id ascending.
A score qualifies when it is above 0 and either fewer than 10 named rows of that game exist or it is strictly greater than the 10th.
A second table `progress`: `game TEXT PRIMARY KEY, reached INTEGER NOT NULL`, created with `CREATE TABLE IF NOT EXISTS` so existing databases gain it without a migration. One row per game with stages, holding the highest stage reached. Written only when a run reaches further than the saved value, where no row counts as 0.

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

## Brick ball

Confirmed with the operator on 2026-10-02. Mockups: https://claude.ai/artifact/88dSJpnwo6tVtUvXK99a8H, the Ember direction.

Classic Breakout: a paddle at the bottom, one or more balls, bricks above. Stages are endless and generated from a formula; there is no level list. Crate `games/brick-ball`, id `brick-ball`, title `brick ball`, about `break every brick · reach a stage to start there`. `stages` is true, the only start key is `enter`. The rules live in a module with no terminal code, like snake's `board.rs`.

### Board

The play area is 60 columns by 22 rows of cells, drawn as 60 by 44 square pixels with half blocks (`▀` and `▄`, foreground and background per cell). Physics runs in floating point pixels; `x` is 0 to 59 and `y` is 0 to 43 with 0 at the top. The brick grid is 15 columns of 4 pixel wide, 1 pixel tall bricks, in 10 rows at `y` 4 to 13. The paddle is 1 pixel tall at `y` 42. The ball is 1 pixel. A ball is lost when its `y` passes 43. A HUD panel of 16 columns, border included, sits to the right of the board. The play screen fits 80 by 24; the program needs 80 by 28 because the SDK game menu is 28 rows tall.

### Stages

Stage `n` is built from a seed derived from the run seed and `n`, so a layout is random per run but repeatable within one. The difficulty depends only on `n`. Reaching a stage, meaning it started with the ball on the paddle, unlocks it; clearing is not needed, so one stage the player cannot pass never blocks progress. `Game::reached` returns the highest stage reached. Variant is `stage <start>-<reached>`. Summary is `▬ <bricks> bricks` with the bricks broken in the run.

Sawtooth: stages run in blocks of 5. The effective stage `e` that feeds every lever is `n - 2` when `n > 1` and `n` is the first stage of a block (`n % 5 == 1`), otherwise `n`. So the first stage of a block eases off from the previous block's peak and the next stages climb past it.

Levers, with `g(e, k) = 1 - exp(-e / k)` rising from 0 towards 1:

| Lever | Value | Floor (e = 0) → cap |
|---|---|---|
| Ball speed, pixels per second | `28 × (1 + 0.7 × g(e, 20))` | 28 → 47.6 |
| Brick count | `round(150 × (0.45 + 0.40 × g(e, 15)))` | 68 → 128 |
| Brick HP | `1 + floor(4 × g(e, 25) + r)`, `r` uniform in [0, 1), never above 5 | all 1 → all 5 |
| Paddle width, pixels | `9 - round(3 × g(e, 30))` | 9 → 6 |
| Unbreakable bricks | none before stage 8, then `round(count × 0.10 × g(e - 8, 20))` of the bricks | 0 → about 10% |

The speed cap keeps the drop from the lowest brick row to the paddle near 0.7 seconds, which is a human reaction limit, so the terminal frame rate never sets the difficulty.

Layout: the seed picks one of six patterns, rows, pyramid, diamond, checker, columns and invader. The pattern is a mask over the 15 by 10 grid, mirrored left to right. The brick count comes from the lever; cells are filled from the mask first and then, if the mask is too small, from empty cells top row first. Unbreakable bricks replace filled cells one at a time, and a replacement is skipped when it would leave a breakable brick unreachable: a flood fill from the empty area under the bricks through every cell that is not unbreakable must reach every breakable brick. Unbreakable bricks do not need clearing. A stage is clear when no breakable brick remains.

Lives: 3 at the start, plus 1 when the player clears a stage whose number is a multiple of 5, never more than 5.

Score: every hit on a breakable brick scores `10 + n`, so a brick with HP `h` is worth `h × (10 + n)` in all. Clearing a stage scores `100 × n`. Starting late earns more per brick with less margin for error, in the Tetris model; the top table is one table and the variant shows where each run started.

### Play

Within one tick every ball moves with the speed read at the start of the tick, so a Slow power-up starting or ending changes the speed only from the next tick on; the ball never stands still or moves two pixels in one frame because of it.

The game ticks every 16.67 milliseconds while a ball is in play, one display refresh at 60 Hz and two at 120 Hz, so frames land evenly on the screen's refresh. While the ball waits on the paddle, during the stage clear countdown and while paused the tick is 33 ms. Every timer counts real seconds, not ticks. Confirmed by play test on 2026-10-06 against a pixel-locked variable tick, which stepped unevenly against the refresh.

Moving objects are drawn with sub-pixel shading. A ball is a 1 pixel square centered on its position; each pixel it overlaps is lit by `c²`, where `c` is the overlapped area, blended from the field 233 toward the ball color 255 through the grey ramp, `233 + round(c² × 22)`, so the main pixel stays bright, the neighbours fall off fast and motion reads as continuous at any speed and angle. Several balls add their shading, capped at the ball color. The faint halo this leaves is accepted: a variant with one full-bright core pixel and `c³` traces was tried on 2026-10-06 and the core stepped visibly, which is what the shading exists to avoid. A pixel holding a brick or the paddle keeps its color unless a ball covers at least half of it. The falling capsule is not blended, because the xterm 256 color cube is too coarse: a pixel it covers by less than half keeps the field color, half or more takes the power color; at 20 pixels per second and 60 ticks per second it steps one pixel every third tick. The ball rests in row 41 at the center of the paddle's middle drawn pixel, the leftmost drawn paddle pixel plus `(w - 1) / 2` rounded down, so it stays on the same paddle pixel while the paddle moves, whatever the paddle width, and is drawn as one full pixel that does not shift when launched. `←` `→` and `a` `d` move the paddle while held: its speed rises evenly from 30 to 90 pixels per second over the first 0.2 seconds of a hold, the paddle position carries fractions between ticks, and the paddle stops at the walls. Brick ball sets `hold_on_press`, so the paddle starts moving on the press in every terminal. While a hold is provisional, that is without release events and before a repeat has confirmed it, the paddle moves at the 30 pixels per second start speed and the ramp begins when the hold is confirmed, so a tap in such a terminal moves the paddle about 12 pixels. Confirmed on 2026-10-06: with the full ramp a tap moved about 21 pixels and felt too sensitive. The ball waits on the paddle at the start of a stage and after a lost life, and `space` launches it straight up with a small random tilt. `p` pauses: nothing moves, the board draws dimmed under a `PAUSED` modal with the stage and score, and `p` resumes. `q` is the SDK's and quits.

Ball movement is sub-stepped so no step moves more than half a pixel, which keeps 1 pixel bricks from being tunnelled. Walls and the ceiling reflect. A brick reflects the ball on the axis it entered through and takes one hit. The paddle sets the new direction from where the ball hit it: `t` from -1 at the left edge to 1 at the right edge gives an angle of `60° × t` from straight up, so the center sends the ball near vertical and the edge at 60°, which is never flatter than 30° from horizontal. Paddle motion does not change the angle. A ball that has not touched the paddle or a breakable brick for 10 seconds is turned by up to 10 degrees, drawn from the board rng, and keeps a vertical component of at least 0.5, so a ball looping between walls and unbreakable bricks keeps turning every 10 seconds until it comes free.

When the last ball is lost the player loses a life. With lives left the ball returns to the paddle. With none, the paddle blinks rust for six half-periods of 200 milliseconds with keys ignored, then the game is over.

Power-ups: when a brick breaks there is an 8% chance it drops a capsule, and only when no capsule is falling. A capsule falls at 20 pixels per second and is caught when it reaches the paddle row over the paddle, taking effect at once with no extra keys; otherwise it is gone past the bottom. `W` wide: the paddle gains 3 pixels for 15 seconds. `S` slow: every ball moves at 0.75 of its speed for 10 seconds. Catching the same kind again restarts its timer. Timers count down only while the ball is in play, so they hold while the ball waits on the paddle and during the stage clear countdown. `M` multi: every ball in play splits into 3 at spread angles, up to 9 balls in all. A life is lost only when the last ball goes.

Stage clear: a `STAGE n CLEAR` modal shows the brick points of the stage, the clear bonus, the score, the stage of the next extra life and `STAGE n+1 IN 2`, counting down 2 seconds, then the next stage starts with the ball on the paddle.

### Look

The board field is 233. Bricks are drawn as 4 pixels, the right one darker so neighbours stay apart. HP 1 to 5 pairs, light then dark: 223/180, 215/173, 209/167, 167/131, 124/88. Unbreakable 245/239. Paddle 173, ball 255. A falling capsule is a 1 by 2 pixel block in its power color, cyan 45 for `W`, lime 82 for `S`, magenta 201 for `M`; the letter appears only on the HUD timer chip. Light bricks are weak and dark bricks are tough, so HP reads without telling hues apart.

HUD, top to bottom: `SCORE` with the score in terracotta, `STAGE` with the number and a meter of 5 `▮` marks for the position in the block, `LIVES` as `●` and `○`, `BEST` with the top name and score, `POWER` with one line per active power-up as a letter chip, a `▰▱` bar and seconds left, and `SPEED` as a multiplier. Key chips `←→`, `␣`, `p`, `q` in the HUD's bottom border, left out when dimmed.

While the ball waits on the paddle the board shows `STAGE n` on one row and `SPACE TO LAUNCH` two rows below, centered, in amber and muted text, left out when dimmed.

Thumbnail: four rows of bricks in red, orange, gold and lime with a gap in the middle of the lower two rows, a 2 by 2 white ball below, and a 7 pixel terracotta paddle near the bottom, as in the mockup.

## Out of scope

The cloud catalog, updating an installed game from a newer catalog copy, a pause in the SDK, sound, stats screen, signing game binaries, running games in a sandbox.
