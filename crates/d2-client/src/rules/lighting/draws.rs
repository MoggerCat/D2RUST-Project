// Spec: specs/render/lighting.md (§11, Edge case 7), specs/render/wall-light-points.tsv
//! The light values handed to the draws (§11): the unit light word (r1),
//! the wall light points and the per-block-column corners (r2), the floor
//! light grid (r3) and the roof grid (r4). `render/shading.md` §3–§4 turns
//! these values into light maps ([`crate::rules::shading`]).
//!
//! Every read goes through [`LightMap::read`] (`0x00475AA0`, x and y in
//! 1/8 sub-tile, clamped to the map).

use std::sync::OnceLock;

use super::map::{LightCell, LightMap};
use crate::rules::blend::wall_block_ops;
use crate::rules::shading::ShadeTables;
use crate::rules::view::{BlockRect, BlockShade};

/// The wall light points of `0x0072A9E8` (normal) and `0x0072ABC8`
/// (faded), directions 1–9, six points each (§11 r2).
pub const WALL_LIGHT_POINTS_TSV: &str =
    include_str!("../../../../../specs/render/wall-light-points.tsv");
/// Light points of a wall with a non-zero direction (`0x006DB9D8`).
pub const WALL_POINTS: usize = 6;
/// Directions the point tables hold (1…9).
pub const WALL_DIRECTIONS: u32 = 9;
/// Cells per axis of the floor / roof light grid (§11 r3).
pub const GRID: usize = 8;
/// DT1 material flag (header `+0x06`) that makes every floor grid byte
/// 0xFF (§11 r3).
pub const MATERIAL_UNLIT: u32 = 0x100;

/// Errors of the draw light values. A case the spec leaves open is an
/// error naming it, never a made-up value (M07).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DrawLightError {
    /// Wall direction 0 (never in 1.14d tiles) as the first wall of its
    /// pass: no earlier record left light words in the pass's array, so
    /// the original's values are undefined (fatal; Edge case 7, OQ8).
    #[error("wall direction 0 with no earlier wall in the pass: the original's light words are undefined (render/lighting.md Edge case 7, OQ8)")]
    WallDirection0,
    /// A direction outside the 1…9 the point tables hold.
    #[error(
        "wall direction {0} is outside the point tables (1…9) of render/wall-light-points.tsv"
    )]
    WallDirection(u32),
    /// A block column whose `c + 1` is not one of the six points (§11 r2
    /// does not say what the draw reads there).
    #[error("wall block x {0}: column c = x >> 5 needs points c and c + 1 of 0…5 (TODO(spec: render/lighting.md §11 r2))")]
    BlockColumn(i32),
    /// A malformed row of `wall-light-points.tsv`.
    #[error("wall-light-points.tsv line {line}: {msg}")]
    Tsv { line: usize, msg: String },
}

/// The light word of a unit draw (§11 r1, `0x004DD600`): the cell of the
/// unit's sub-tile, read at sub-tile × 8, as `B << 24 | G << 16 | R << 8 |
/// I`.
pub fn unit_light(map: &LightMap, sub_tile: (i32, i32)) -> u32 {
    map.read(sub_tile.0 * 8, sub_tile.1 * 8).word()
}

/// The GDI light byte `v` of a light word: its low byte (§11 r1,
/// `render/shading.md` §3).
pub fn light_byte(word: u32) -> u8 {
    (word & 0xFF) as u8
}

/// Which point table a wall reads (§11 r2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PointTable {
    /// `0x0072A9E8`.
    Normal,
    /// `0x0072ABC8`: the record's fade state bit 0 is set.
    Faded,
}

impl PointTable {
    /// The table of a wall record with fade state `fade`
    /// (`render/draw-order.md` §8): bit 0 selects the faded table.
    pub fn of_fade(fade: u32) -> PointTable {
        if fade & 1 != 0 {
            PointTable::Faded
        } else {
            PointTable::Normal
        }
    }

    fn index(self) -> usize {
        match self {
            PointTable::Normal => 0,
            PointTable::Faded => 1,
        }
    }
}

/// One row of `wall-light-points.tsv`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WallPointRow {
    pub direction: u32,
    pub table: PointTable,
    pub point: usize,
    pub dx: i32,
    pub dy: i32,
}

/// The two point tables, directions 1…9, six `(dx, dy)` each (§11 r2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WallPoints {
    /// `[table][direction − 1][point]`.
    points: [[[(i32, i32); WALL_POINTS]; WALL_DIRECTIONS as usize]; 2],
    rows: Vec<WallPointRow>,
}

fn tsv_err(line: usize, msg: impl Into<String>) -> DrawLightError {
    DrawLightError::Tsv {
        line,
        msg: msg.into(),
    }
}

impl WallPoints {
    /// Parses the TSV strictly: the header `direction table point dx dy`,
    /// five tab-separated fields per row, direction 1…9, table `normal` or
    /// `faded`, point 0…5, `dx` / `dy` integers, and every
    /// (direction, table, point) exactly once.
    pub fn parse(text: &str) -> Result<WallPoints, DrawLightError> {
        let mut lines = text.lines().enumerate();
        match lines.next() {
            Some((_, "direction\ttable\tpoint\tdx\tdy")) => {}
            _ => return Err(tsv_err(1, "header is not `direction table point dx dy`")),
        }
        let mut seen = [[[false; WALL_POINTS]; WALL_DIRECTIONS as usize]; 2];
        let mut points = [[[(0, 0); WALL_POINTS]; WALL_DIRECTIONS as usize]; 2];
        let mut rows = Vec::new();
        for (n, line) in lines {
            let line_no = n + 1;
            let f: Vec<&str> = line.split('\t').collect();
            if f.len() != 5 {
                return Err(tsv_err(line_no, format!("{} fields, expected 5", f.len())));
            }
            let int = |s: &str, what: &str| {
                s.parse::<i32>()
                    .map_err(|_| tsv_err(line_no, format!("{what} `{s}` is not an integer")))
            };
            let direction = int(f[0], "direction")?;
            if !(1..=WALL_DIRECTIONS as i32).contains(&direction) {
                return Err(tsv_err(
                    line_no,
                    format!("direction {direction} not in 1…9"),
                ));
            }
            let table = match f[1] {
                "normal" => PointTable::Normal,
                "faded" => PointTable::Faded,
                t => return Err(tsv_err(line_no, format!("table `{t}`"))),
            };
            let point = int(f[2], "point")?;
            if !(0..WALL_POINTS as i32).contains(&point) {
                return Err(tsv_err(line_no, format!("point {point} not in 0…5")));
            }
            let (dx, dy) = (int(f[3], "dx")?, int(f[4], "dy")?);
            let (d, p) = (direction as usize - 1, point as usize);
            let slot = &mut seen[table.index()][d][p];
            if *slot {
                return Err(tsv_err(line_no, "duplicate (direction, table, point)"));
            }
            *slot = true;
            points[table.index()][d][p] = (dx, dy);
            rows.push(WallPointRow {
                direction: direction as u32,
                table,
                point: p,
                dx,
                dy,
            });
        }
        if seen.iter().flatten().flatten().any(|s| !s) {
            return Err(tsv_err(
                text.lines().count(),
                "missing rows: need 9 directions × 2 tables × 6 points",
            ));
        }
        Ok(WallPoints { points, rows })
    }

    /// The rows in file order.
    pub fn rows(&self) -> &[WallPointRow] {
        &self.rows
    }

    /// The six `(dx, dy)` of a wall with DT1 `direction` (header `+0x00`)
    /// in `table`. Direction 0 has no points: [`DrawLightError::WallDirection0`]
    /// here; [`WallPass`] gives it the previous record's words.
    pub fn offsets(
        &self,
        direction: u32,
        table: PointTable,
    ) -> Result<[(i32, i32); WALL_POINTS], DrawLightError> {
        match direction {
            0 => Err(DrawLightError::WallDirection0),
            d if d <= WALL_DIRECTIONS => Ok(self.points[table.index()][d as usize - 1]),
            d => Err(DrawLightError::WallDirection(d)),
        }
    }
}

/// The parsed `wall-light-points.tsv` (the file is part of the spec; a
/// parse failure is a broken checkout).
pub fn wall_points() -> &'static WallPoints {
    static POINTS: OnceLock<WallPoints> = OnceLock::new();
    POINTS.get_or_init(|| {
        WallPoints::parse(WALL_LIGHT_POINTS_TSV).expect("specs/render/wall-light-points.tsv parses")
    })
}

/// The origin sub-tile of a wall / floor tile (`0x004DD180`): the room's
/// sub-tile origin + 5 × the record's tile position.
pub fn tile_origin_sub_tile(room_sub_tile: (i32, i32), tile: (i32, i32)) -> (i32, i32) {
    (room_sub_tile.0 + 5 * tile.0, room_sub_tile.1 + 5 * tile.1)
}

/// The six light words of a wall or lower wall (§11 r2): with `(X, Y)` = 8
/// × the tile's origin sub-tile, point `p` reads the cell at `(X + 8·dx_p,
/// Y + 8·dy_p)`. `fade` is the record's fade state (bit 0 → the faded
/// table).
pub fn wall_light_words(
    map: &LightMap,
    origin_sub_tile: (i32, i32),
    direction: u32,
    fade: u32,
) -> Result<[u32; WALL_POINTS], DrawLightError> {
    let offsets = wall_points().offsets(direction, PointTable::of_fade(fade))?;
    let (x, y) = (8 * origin_sub_tile.0, 8 * origin_sub_tile.1);
    Ok(offsets.map(|(dx, dy)| map.read(x + 8 * dx, y + 8 * dy).word()))
}

/// The light array of one wall pass (`0x004DF1C0` loop, §11 r2, OQ8):
/// one array for the whole pass, so a record with direction 0 (no light
/// points; never in 1.14d tiles) keeps the words the previous record of
/// the pass wrote. Start a new one per pass.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WallPass {
    last: Option<[u32; WALL_POINTS]>,
}

impl WallPass {
    /// [`wall_light_words`] for the next record of the pass; direction 0
    /// reuses the previous record's words, and is
    /// [`DrawLightError::WallDirection0`] when it is the pass's first.
    pub fn words(
        &mut self,
        map: &LightMap,
        origin_sub_tile: (i32, i32),
        direction: u32,
        fade: u32,
    ) -> Result<[u32; WALL_POINTS], DrawLightError> {
        if direction == 0 {
            return self.last.ok_or(DrawLightError::WallDirection0);
        }
        let w = wall_light_words(map, origin_sub_tile, direction, fade)?;
        self.last = Some(w);
        Ok(w)
    }
}

/// The corners `c0…c3` of the wall block at block x `block_x` (§11 r2):
/// column `c = block_x >> 5`, `c0 = c3 = I_c`, `c1 = c2 = I_{c+1}` (low
/// bytes of the words). A column whose `c + 1` is past point 5 (block x ≥
/// 160) or negative is [`DrawLightError::BlockColumn`]: the spec does not
/// say what the draw reads there.
pub fn wall_block_corners(
    words: &[u32; WALL_POINTS],
    block_x: i32,
) -> Result<[u8; 4], DrawLightError> {
    let c = block_x >> 5;
    if c < 0 || c as usize + 1 >= WALL_POINTS {
        return Err(DrawLightError::BlockColumn(block_x));
    }
    let (a, b) = (
        light_byte(words[c as usize]),
        light_byte(words[c as usize + 1]),
    );
    Ok([a, b, b, a])
}

/// The shaded blocks of a wall or roof tile (§11 r2 corners through
/// `blend-modes.md` §6 / `shading.md` §4 walls, [`wall_block_ops`]): per
/// block, its corners from `words` at the block's x, then the lit
/// (opaque) or translucent ops of wall alpha `alpha`. Blocks the wall
/// alpha hides are left out. The gradient position is (0, 0); the view
/// moves it to the block.
pub fn wall_block_shades(
    tables: &ShadeTables,
    words: &[u32; WALL_POINTS],
    blocks: &[BlockRect],
    alpha: u8,
    low_quality: bool,
) -> Result<Vec<BlockShade>, DrawLightError> {
    let mut out = Vec::with_capacity(blocks.len());
    for block in blocks {
        let corners = wall_block_corners(words, block.x)?;
        if let Some((shade, blend)) = wall_block_ops(tables, alpha, corners, low_quality, 0, 0) {
            out.push(BlockShade {
                block: *block,
                shade,
                blend,
            });
        }
    }
    Ok(out)
}

/// The 8 × 8 floor / roof light grid (§11 r3): row-major, cell `(i, j)`
/// holds bytes 0–3 = I, R, G, B (the original's cells are 12 bytes; the
/// other 8 are not read by `render/shading.md` §4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LightGrid {
    /// `cells[j * 8 + i]` = `[I, R, G, B]`.
    pub cells: [[u8; 4]; GRID * GRID],
}

impl LightGrid {
    /// Every cell `[I, R, G, B]` = `value`.
    pub fn filled(value: [u8; 4]) -> LightGrid {
        LightGrid {
            cells: [value; GRID * GRID],
        }
    }

    /// Cell `(i, j)` (column `i`, row `j`, 0…7).
    pub fn cell(&self, i: usize, j: usize) -> [u8; 4] {
        self.cells[j * GRID + i]
    }

    /// Byte 0 of every cell in row-major order: the `e[n]` that
    /// [`crate::rules::shading::floor_block_light`] reads.
    pub fn light_values(&self) -> [u8; GRID * GRID] {
        self.cells.map(|c| c[0])
    }

    /// Reads cell `(i, j)` = sub-tile `(sx − 1 + i, sy − 1 + j)`.
    fn read(map: &LightMap, sub_tile: (i32, i32)) -> LightGrid {
        let mut grid = LightGrid::filled([0; 4]);
        for j in 0..GRID {
            for i in 0..GRID {
                let c = map.read(
                    8 * (sub_tile.0 - 1 + i as i32),
                    8 * (sub_tile.1 - 1 + j as i32),
                );
                grid.cells[j * GRID + i] = [c.i, c.r, c.g, c.b];
            }
        }
        grid
    }
}

/// The floor light grid of a floor tile (§11 r3, `0x004DE410` →
/// `0x004DDEF0`): cell `(i, j)` = I, R, G, B of sub-tile `(sx − 1 + i, sy −
/// 1 + j)`, `(sx, sy)` the tile's origin sub-tile; DT1 material flag 0x100
/// (header `+0x06`) → every byte 0xFF.
pub fn floor_light_grid(map: &LightMap, origin_sub_tile: (i32, i32), material: u32) -> LightGrid {
    if material & MATERIAL_UNLIT != 0 {
        return LightGrid::filled([0xFF; 4]);
    }
    LightGrid::read(map, origin_sub_tile)
}

/// The light grid of a roof (§11 r4, `0x004DEA70`): the floor grid at the
/// roof record's sub-tile (draw entry `+4/+8 >> 3`, `draw_pos` in 1/8
/// sub-tile); roof height (header `+0x04`) ≠ 0 → every cell is the act
/// environment's I, R, G, B (§9).
pub fn roof_light_grid(
    map: &LightMap,
    draw_pos: (i32, i32),
    height: u32,
    environment: LightCell,
) -> LightGrid {
    if height != 0 {
        let e = environment;
        return LightGrid::filled([e.i, e.r, e.g, e.b]);
    }
    LightGrid::read(map, (draw_pos.0 >> 3, draw_pos.1 >> 3))
}
