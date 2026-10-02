//! Stage layouts and difficulty levers. No terminal code here.

use std::collections::VecDeque;

use rand::{RngExt, SeedableRng, rngs::StdRng};

pub const COLS: usize = 15;
pub const ROWS: usize = 10;
/// Stages per sawtooth block.
pub const BLOCK: u32 = 5;
pub const MAX_HP: u8 = 5;
/// Ball speed lever at e = 0, px/s. The HUD speed multiplier divides by this.
pub const BASE_SPEED: f64 = 28.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cell {
    Empty,
    Brick(u8),
    Unbreakable,
}

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
pub fn g(e: f64, k: f64) -> f64 {
    1.0 - (-e / k).exp()
}

/// n - 2 when n > 1 and n % BLOCK == 1, else n.
pub fn effective(n: u32) -> u32 {
    if n > 1 && n % BLOCK == 1 { n - 2 } else { n }
}

/// Every lever for effective stage e.
pub fn levers(e: u32) -> Levers {
    let x = e as f64;
    let count = (150.0 * (0.45 + 0.40 * g(x, 15.0))).round() as usize;
    Levers {
        e,
        speed: BASE_SPEED * (1.0 + 0.7 * g(x, 20.0)),
        count,
        paddle: 9 - (3.0 * g(x, 30.0)).round() as u32,
        unbreakable: if e < 8 {
            0
        } else {
            (count as f64 * 0.10 * g((e - 8) as f64, 20.0)).round() as usize
        },
    }
}

/// 1 + floor(4 × g(e, 25) + r), capped at MAX_HP. r in [0, 1).
pub fn hp(e: u32, r: f64) -> u8 {
    (1 + (4.0 * g(e as f64, 25.0) + r).floor() as u8).min(MAX_HP)
}

/// Seed of stage n in a run.
pub fn stage_seed(run: u64, n: u32) -> u64 {
    run ^ (n as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
}

/// Pattern masks, left 8 columns (0 to 7, column 7 is the center). Column c
/// reads PATTERNS[p][r][min(c, 14 - c)], so every mask is mirrored left to right.
#[rustfmt::skip]
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

/// Order in which a row's empty cells are filled once the mask is used up:
/// center outward, so the layout stays near symmetric.
const FILL_ORDER: [usize; COLS] = [7, 6, 8, 5, 9, 4, 10, 3, 11, 2, 12, 1, 13, 0, 14];

/// True when pattern p has a brick at (c, r).
pub fn mask(p: usize, c: usize, r: usize) -> bool {
    PATTERNS[p][r].as_bytes()[c.min(COLS - 1 - c)] == b'#'
}

/// Every (row, col) in row-major order.
fn cells() -> impl Iterator<Item = (usize, usize)> {
    (0..ROWS).flat_map(|r| (0..COLS).map(move |c| (r, c)))
}

/// The layout of stage n of the run with seed `run`.
pub fn build(run: u64, n: u32) -> Grid {
    let mut rng = StdRng::seed_from_u64(stage_seed(run, n));
    let lv = levers(effective(n));
    let p = rng.random_range(0..PATTERNS.len());

    let mut filled = [[false; COLS]; ROWS];
    for (r, c) in cells() {
        filled[r][c] = mask(p, c, r);
    }
    let in_mask = filled.iter().flatten().filter(|&&f| f).count();
    let extra: Vec<_> = (0..ROWS)
        .flat_map(|r| FILL_ORDER.map(|c| (r, c)))
        .filter(|&(r, c)| !filled[r][c])
        .take(lv.count.saturating_sub(in_mask))
        .collect();
    for (r, c) in extra {
        filled[r][c] = true;
    }

    let mut grid = [[Cell::Empty; COLS]; ROWS];
    for (r, c) in cells().filter(|&(r, c)| filled[r][c]) {
        grid[r][c] = Cell::Brick(hp(lv.e, rng.random::<f64>()));
    }

    let mut candidates: Vec<_> = cells().filter(|&(r, c)| filled[r][c]).collect();
    for i in (1..candidates.len()).rev() {
        candidates.swap(i, rng.random_range(0..=i));
    }
    let mut placed = 0;
    for (r, c) in candidates {
        if placed == lv.unbreakable {
            break;
        }
        let brick = grid[r][c];
        grid[r][c] = Cell::Unbreakable;
        if all_reachable(&grid) {
            placed += 1;
        } else {
            grid[r][c] = brick;
        }
    }
    grid
}

/// True when a flood fill from the open area under the grid, through every
/// cell that is not Unbreakable, reaches every Brick.
pub fn all_reachable(grid: &Grid) -> bool {
    // Row 0 is the lane above the bricks, rows 1 to ROWS the brick rows and
    // the last row the open area under them.
    const H: usize = ROWS + 2;
    let passable = |r: usize, c: usize| r == 0 || r == H - 1 || grid[r - 1][c] != Cell::Unbreakable;
    let mut seen = [[false; COLS]; H];
    seen[H - 1] = [true; COLS];
    let mut queue: VecDeque<_> = (0..COLS).map(|c| (H - 1, c)).collect();
    while let Some((r, c)) = queue.pop_front() {
        let next = [
            (r.wrapping_sub(1), c),
            (r + 1, c),
            (r, c.wrapping_sub(1)),
            (r, c + 1),
        ];
        for (r, c) in next {
            if r < H && c < COLS && !seen[r][c] && passable(r, c) {
                seen[r][c] = true;
                queue.push_back((r, c));
            }
        }
    }
    cells().all(|(r, c)| !matches!(grid[r][c], Cell::Brick(_)) || seen[r + 1][c])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn count(grid: &Grid, f: impl Fn(Cell) -> bool) -> usize {
        grid.iter().flatten().filter(|&&cell| f(cell)).count()
    }

    #[test]
    fn levers_match_the_spec_floor_and_cap() {
        let lo = levers(0);
        assert!((lo.speed - BASE_SPEED).abs() < 1e-9, "{}", lo.speed);
        assert_eq!((lo.count, lo.paddle, lo.unbreakable), (68, 9, 0));
        let hi = levers(1000);
        assert!((hi.speed - 47.6).abs() < 1e-9, "{}", hi.speed);
        assert_eq!((hi.count, hi.paddle, hi.unbreakable), (128, 6, 13));
    }

    #[test]
    fn levers_stay_between_floor_and_cap_and_rise_with_e() {
        for e in 0..=500 {
            let lv = levers(e);
            assert!(28.0 <= lv.speed && lv.speed <= 47.6 + 1e-9, "e {e}: {lv:?}");
            assert!((68..=128).contains(&lv.count), "e {e}: {lv:?}");
            assert!((6..=9).contains(&lv.paddle), "e {e}: {lv:?}");
            assert!(lv.unbreakable <= 13, "e {e}: {lv:?}");
        }
        for e in 0..500 {
            let (a, b) = (levers(e), levers(e + 1));
            assert!(a.speed <= b.speed, "e {e}: {a:?} {b:?}");
            assert!(a.count <= b.count, "e {e}: {a:?} {b:?}");
            assert!(a.paddle >= b.paddle, "e {e}: {a:?} {b:?}");
            if e >= 8 {
                assert!(a.unbreakable <= b.unbreakable, "e {e}: {a:?} {b:?}");
            }
        }
    }

    #[test]
    fn levers_match_the_reference_table() {
        let table = [
            (1, 1, 28.956, 71, 9, 0),
            (5, 5, 32.336, 85, 9, 0),
            (6, 4, 31.553, 82, 9, 0),
            (10, 10, 35.712, 97, 8, 1),
            (20, 20, 40.390, 112, 8, 5),
            (1000, 1000, 47.6, 128, 6, 13),
        ];
        for (n, e, speed, count, paddle, unbreakable) in table {
            assert_eq!(effective(n), e, "n {n}");
            let lv = levers(e);
            assert!((lv.speed - speed).abs() < 0.001, "n {n}: {lv:?}");
            assert_eq!(
                (lv.count, lv.paddle, lv.unbreakable),
                (count, paddle, unbreakable),
                "n {n}"
            );
        }
    }

    #[test]
    fn sawtooth_eases_the_first_stage_of_a_block() {
        assert_eq!(effective(1), 1);
        assert_eq!(effective(5), 5);
        assert_eq!(effective(6), 4);
        assert_eq!(effective(7), 7);
        assert_eq!(effective(11), 9);
    }

    #[test]
    fn hp_is_all_one_at_the_floor_and_reaches_five_at_the_cap() {
        for i in 0..100 {
            assert_eq!(hp(0, i as f64 / 100.0), 1, "r {i}/100");
        }
        assert_eq!(hp(1, 0.0), 1);
        assert_eq!(hp(1, 0.9), 2);
        assert_eq!(hp(25, 0.0), 3);
        assert_eq!(hp(25, 0.5), 4);
        assert_eq!(hp(1000, 0.0), 5);
        assert_eq!(hp(1000, 0.999), 5);
        for seed in 0..10 {
            let grid = build(seed, 200);
            assert!(
                grid.iter().flatten().any(|&cell| cell == Cell::Brick(5)),
                "seed {seed}"
            );
            for &cell in grid.iter().flatten() {
                if let Cell::Brick(h) = cell {
                    assert!(h == 4 || h == 5, "seed {seed}: {cell:?}");
                }
            }
        }
    }

    #[test]
    fn masks_read_the_mirrored_column_and_fit_stage_one() {
        let stage_one = levers(effective(1)).count;
        for (p, want) in [65, 64, 56, 68, 70, 46].into_iter().enumerate() {
            for row in PATTERNS[p] {
                assert_eq!(row.len(), 8, "pattern {p}: {row:?}");
                assert!(
                    row.chars().all(|ch| ch == '#' || ch == '.'),
                    "pattern {p}: {row:?}"
                );
            }
            let cells = cells().filter(|&(r, c)| mask(p, c, r)).count();
            assert_eq!(cells, want, "pattern {p}");
            assert!(cells <= stage_one, "pattern {p}");
        }
        assert!(mask(1, 0, 9) && mask(1, 14, 9));
        assert!(mask(1, 7, 2) && !mask(1, 6, 2) && !mask(1, 8, 2));
        assert!(mask(5, 4, 1) && mask(5, 10, 1) && !mask(5, 7, 1));
    }

    #[test]
    fn same_seed_gives_the_same_layout() {
        assert_eq!(build(42, 7), build(42, 7));
        assert_eq!(build(42, 23), build(42, 23));
        assert!((0..10).any(|s| build(s, 7) != build(42, 7)));
    }

    #[test]
    fn stages_with_the_same_levers_differ_within_a_run() {
        // Stages 4 and 6 share e = 4, and 9 and 11 share e = 9, so only the
        // stage seed tells their layouts apart.
        assert_eq!(effective(4), effective(6));
        assert_eq!(effective(9), effective(11));
        assert_ne!(build(42, 4), build(42, 6));
        assert_ne!(build(42, 9), build(42, 11));
    }

    #[test]
    fn extra_bricks_fill_row_0_from_the_center_out() {
        // The pyramid mask leaves rows 0 and 1 empty and holds 64 bricks, so
        // stage 1, with 71, puts its 7 extra bricks in row 0, columns 4 to 10.
        // build picks the pattern with the first draw from the stage seed.
        let pyramid =
            |run| StdRng::seed_from_u64(stage_seed(run, 1)).random_range(0..PATTERNS.len()) == 1;
        let grid = build((0..).find(|&run| pyramid(run)).unwrap(), 1);
        let row0: Vec<_> = (0..COLS).filter(|&c| grid[0][c] != Cell::Empty).collect();
        assert_eq!(row0, (4..=10).collect::<Vec<_>>());
        assert!(grid[1].iter().all(|&cell| cell == Cell::Empty));
    }

    #[test]
    fn brick_count_matches_the_lever() {
        for n in [1, 2, 5, 6, 9, 10, 15, 20, 37, 60, 200] {
            let lv = levers(effective(n));
            for seed in 0..20 {
                let grid = build(seed, n);
                assert_eq!(
                    count(&grid, |c| c != Cell::Empty),
                    lv.count,
                    "n {n} seed {seed}"
                );
                assert!(
                    count(&grid, |c| c == Cell::Unbreakable) <= lv.unbreakable,
                    "n {n} seed {seed}"
                );
            }
        }
    }

    #[test]
    fn every_breakable_brick_is_reachable() {
        for seed in 0..200 {
            for n in [8, 10, 13, 20, 40, 80, 200] {
                assert!(all_reachable(&build(seed, n)), "n {n} seed {seed}");
            }
        }
    }

    #[test]
    fn reachability_finds_a_sealed_brick() {
        let mut grid = [[Cell::Empty; COLS]; ROWS];
        grid[1][7] = Cell::Brick(1);
        for (c, r) in [(7, 0), (6, 1), (8, 1), (7, 2)] {
            grid[r][c] = Cell::Unbreakable;
        }
        assert!(!all_reachable(&grid));
        grid[2][7] = Cell::Empty;
        assert!(all_reachable(&grid));
    }

    #[test]
    fn no_unbreakable_before_stage_8() {
        for n in 1..=7 {
            for seed in 0..50 {
                assert_eq!(
                    count(&build(seed, n), |c| c == Cell::Unbreakable),
                    0,
                    "n {n} seed {seed}"
                );
            }
        }
    }

    #[test]
    fn unbreakables_appear_by_stage_20() {
        for seed in 0..20 {
            assert!(
                count(&build(seed, 20), |c| c == Cell::Unbreakable) > 0,
                "seed {seed}"
            );
        }
    }
}
