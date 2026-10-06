// Spec: specs/render/lighting.md (§1, §2, §3, §4, §11 intro, §12)
//! The light map (§1): 48 × 48 cells of 8 bytes around the local player,
//! the ambient fill (§3), the blocks-light flags (§4), the build order
//! (§2) and the frame-end digest (§12 r3).

use sha2::{Digest, Sha256};

use super::records::{LightError, LightList, LightWorld};

/// Cells per axis (§1 r1).
pub const MAP_SIZE: i32 = 48;
/// The window half-width: origin = player sub-tile − 24 (§1 r2).
pub const HALF: i32 = 24;
/// Bytes of the map as 1.14d stores it (`0x007B0E68`, §12 r3).
pub const MAP_BYTES: usize = (MAP_SIZE * MAP_SIZE) as usize * 8;

/// One light-map cell (§1 r1): `+0` u32 blocks-light flag, `+4` intensity
/// `I`, `+5` R, `+6` G, `+7` B.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct LightCell {
    pub blocks: u32,
    pub i: u8,
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl LightCell {
    /// The dword a draw receives (§11 r1): `B << 24 | G << 16 | R << 8 | I`.
    pub fn word(&self) -> u32 {
        u32::from(self.b) << 24 | u32::from(self.g) << 16 | u32::from(self.r) << 8 | u32::from(self.i)
    }
}

/// The light map (§1). Cell `(gx, gy)` is sub-tile `(ox + gx, oy + gy)`,
/// row-major (row = y).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LightMap {
    /// `(ox, oy)` = the local player's sub-tile − 24 (§1 r2).
    pub origin: (i32, i32),
    cells: Vec<LightCell>,
}

impl LightMap {
    /// A map at the origin of a player at sub-tile `player` (§1 r2), every
    /// cell zero.
    pub fn new(player: (i32, i32)) -> Self {
        LightMap {
            origin: (player.0 - HALF, player.1 - HALF),
            cells: vec![LightCell::default(); (MAP_SIZE * MAP_SIZE) as usize],
        }
    }

    /// The inclusive upper bound of the source window test (§1 r2):
    /// origin + 48 = player + 24.
    pub fn upper(&self) -> (i32, i32) {
        (self.origin.0 + MAP_SIZE, self.origin.1 + MAP_SIZE)
    }

    fn index(gx: i32, gy: i32) -> Option<usize> {
        ((0..MAP_SIZE).contains(&gx) && (0..MAP_SIZE).contains(&gy))
            .then(|| (gy * MAP_SIZE + gx) as usize)
    }

    /// Cell `(gx, gy)` in map coordinates, `None` outside 0…47.
    pub fn cell(&self, gx: i32, gy: i32) -> Option<&LightCell> {
        Self::index(gx, gy).map(|i| &self.cells[i])
    }

    /// Mutable cell `(gx, gy)`; writes outside 0…47 have no cell (§1 r2:
    /// dropped).
    pub fn cell_mut(&mut self, gx: i32, gy: i32) -> Option<&mut LightCell> {
        Self::index(gx, gy).map(|i| &mut self.cells[i])
    }

    /// Every cell, row-major.
    pub fn cells(&self) -> &[LightCell] {
        &self.cells
    }

    /// Every cell, row-major, mutable.
    pub fn cells_mut(&mut self) -> &mut [LightCell] {
        &mut self.cells
    }

    /// The light-map read of a draw (`0x00475AA0`, §11 intro): `x`, `y` in
    /// 1/8 sub-tile; the cell `(x >> 3) − origin` is clamped to 0…47 on
    /// each axis.
    pub fn read(&self, x: i32, y: i32) -> LightCell {
        let gx = ((x >> 3) - self.origin.0).clamp(0, MAP_SIZE - 1);
        let gy = ((y >> 3) - self.origin.1).clamp(0, MAP_SIZE - 1);
        self.cells[(gy * MAP_SIZE + gx) as usize]
    }

    /// The map's bytes as 1.14d holds them at `0x007B0E68` (§12 r3): per
    /// cell the u32 flag little-endian, then I, R, G, B.
    pub fn bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(MAP_BYTES);
        for c in &self.cells {
            out.extend_from_slice(&c.blocks.to_le_bytes());
            out.extend_from_slice(&[c.i, c.r, c.g, c.b]);
        }
        out
    }

    /// SHA-256 of [`Self::bytes`] (§12 r3): the light-map part of a
    /// capture's state key.
    pub fn digest(&self) -> [u8; 32] {
        Sha256::digest(self.bytes()).into()
    }

    /// Ambient fill (§3, `0x00474610`). `None`: no local player or no
    /// player room, every cell zero (r1). Otherwise every cell := the
    /// player room's ambient with flag 0 (r2), then each near room in list
    /// order except the player's room fills its rectangle plus one column
    /// and one row (r3).
    pub fn fill_ambient(&mut self, scene: Option<&AmbientScene>) {
        let Some(scene) = scene else {
            self.cells.fill(LightCell::default());
            return;
        };
        self.cells.fill(scene.player_ambient.cell());
        let (ox, oy) = self.origin;
        for room in scene.near.iter().filter(|r| !r.is_player_room) {
            let (x, y, w, h) = room.rect;
            let (xp, yp) = (x - ox, y - oy);
            if xp > MAP_SIZE || xp + w < 0 || yp > MAP_SIZE || yp + h < 0 {
                continue;
            }
            let fill = room.ambient.cell();
            for gy in yp.max(0)..=(yp + h).min(MAP_SIZE - 1) {
                for gx in xp.max(0)..=(xp + w).min(MAP_SIZE - 1) {
                    if let Some(c) = self.cell_mut(gx, gy) {
                        *c = fill;
                    }
                }
            }
        }
    }

    /// Blocks-light flags (§4, `0x004756D0`): for each cell, rows then
    /// columns, flag := 1 when `blocks(sub-tile x, sub-tile y)` (the
    /// collision point test with mask 0x22 is non-zero); else unchanged.
    /// The caller runs this only when the player and its room exist.
    pub fn fill_blocks(&mut self, mut blocks: impl FnMut(i32, i32) -> bool) {
        let (ox, oy) = self.origin;
        for gy in 0..MAP_SIZE {
            for gx in 0..MAP_SIZE {
                if blocks(ox + gx, oy + gy) {
                    self.cells[(gy * MAP_SIZE + gx) as usize].blocks = 1;
                }
            }
        }
    }

    /// The build of one drawn frame (§2, `0x00475800`): origin at
    /// `player` (r1), ambient fill (r2), blocks-light flags when `scene`
    /// exists (r3), then every record head → tail with quality `q` (r4
    /// is the caller's: `rules::lighting::quality`; r5).
    pub fn build(
        player: (i32, i32),
        scene: Option<&AmbientScene>,
        blocks: impl FnMut(i32, i32) -> bool,
        q: u8,
        lights: &mut LightList,
        world: &impl LightWorld,
    ) -> Result<LightMap, LightError> {
        let mut map = LightMap::new(player);
        map.fill_ambient(scene);
        if scene.is_some() {
            map.fill_blocks(blocks);
        }
        lights.frame(&mut map, q, world)?;
        Ok(map)
    }
}

/// A room ambient `(I, R, G, B)` (§3.1, computed by the caller).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Ambient {
    pub i: u8,
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Ambient {
    fn cell(self) -> LightCell {
        LightCell {
            blocks: 0,
            i: self.i,
            r: self.r,
            g: self.g,
            b: self.b,
        }
    }
}

/// One room of the player room's near list (§3 r3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NearRoom {
    /// Sub-tile rectangle `(x, y, w, h)` (room `+0x4C..+0x58`).
    pub rect: (i32, i32, i32, i32),
    pub ambient: Ambient,
    /// This entry is the player's room (the pointer compare): skipped.
    pub is_player_room: bool,
}

/// The ambient input of §3 when the local player and its room exist.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AmbientScene {
    /// The ambient of the player's room (§3 r2).
    pub player_ambient: Ambient,
    /// The player room's near list, in list order (§3 r3).
    pub near: Vec<NearRoom>,
}
