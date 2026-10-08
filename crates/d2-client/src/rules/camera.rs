// Spec: specs/render/camera.md
//! World position → screen draw position: frame size and play area (§1),
//! client pixels (§2), the camera origins of one drawn frame (§3), unit
//! and tile draw positions (§4–§6), view culling (§7), screen shake (§8)
//! and the time base (§9). Integer math only; every division and shift is
//! the original's (C division on positive sizes, arithmetic shifts on
//! signed pixel sums).

use d2_sim::rng::Seed;

use crate::scene::Rect;

/// Rows below the play area (§1: play-area height `H − 40`).
pub const PANEL_HEIGHT: i32 = 40;
/// Client tick in milliseconds (§9, `0x0070EF1C`).
pub const TICK_MS: u32 = 40;
/// Tile cell size in client pixels (§2): `sx = (tx − ty) × 80`.
pub const CELL_HALF_WIDTH: i32 = 80;
pub const CELL_HALF_HEIGHT: i32 = 40;

/// The frame W × H (§1). d2rs draws 800 × 600 by default (EARLY_DECISIONS
/// 9); `play --res 640x480` runs the 640 × 480 frame ([`FrameSize::play`]),
/// and the rules are written for any size so the spec's 640 × 480 vectors
/// run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FrameSize {
    pub width: i32,
    pub height: i32,
}

impl FrameSize {
    /// Resolution mode 2, the d2rs default.
    pub const D2RS: FrameSize = FrameSize {
        width: 800,
        height: 600,
    };
    /// Resolution mode 0 (`play --res 640x480`; the spec's 640 × 480 vectors).
    pub const LOW: FrameSize = FrameSize {
        width: 640,
        height: 480,
    };

    /// Play-area height `H − 40` (§1).
    pub fn play_height(&self) -> i32 {
        self.height - PANEL_HEIGHT
    }

    /// The frame `[0, W) × [0, H)`: the clip of every world draw (§10).
    pub fn rect(&self) -> Rect {
        Rect::new(0, 0, self.width as u32, self.height as u32)
    }

    /// The frame the play path draws: [`FrameSize::D2RS`] unless
    /// [`FrameSize::set_play`] chose another before the app started.
    /// d2rs-own (decision q-fix-ui-draw-sink): the original keeps its
    /// display size in globals set once per video mode
    /// (`GeneralDisplayWidth/Height`, §1); d2rs sets it once per process
    /// from `play --res`, so the 640 × 480 rules run in play.
    pub fn play() -> FrameSize {
        PLAY_FRAME.get().copied().unwrap_or(FrameSize::D2RS)
    }

    /// Chooses the play frame (resolution mode 0 or 2 of §1) once per
    /// process; a second call with another size is refused (the frame of a
    /// running app never changes).
    pub fn set_play(size: FrameSize) -> Result<(), PlayFrameError> {
        if size != FrameSize::D2RS && size != FrameSize::LOW {
            return Err(PlayFrameError::Unsupported(size));
        }
        let now = *PLAY_FRAME.get_or_init(|| size);
        if now != size {
            return Err(PlayFrameError::AlreadySet { now, asked: size });
        }
        Ok(())
    }
}

/// The play frame of [`FrameSize::play`].
static PLAY_FRAME: std::sync::OnceLock<FrameSize> = std::sync::OnceLock::new();

/// Errors of [`FrameSize::set_play`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum PlayFrameError {
    #[error("frame {0:?} is neither 800 × 600 nor 640 × 480 (§1 resolution modes 2, 0)")]
    Unsupported(FrameSize),
    #[error("the play frame is already {now:?}; {asked:?} refused")]
    AlreadySet { now: FrameSize, asked: FrameSize },
}

/// The screen open mode 0–3 (§1, `D2Client_ScreenOpenMode`); which panels
/// set which mode is `ui/panels.md`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct OpenMode(u8);

impl OpenMode {
    pub const NONE: OpenMode = OpenMode(0);

    /// `None` outside 0–3.
    pub fn new(mode: u8) -> Option<Self> {
        (mode <= 3).then_some(OpenMode(mode))
    }

    pub fn get(&self) -> u8 {
        self.0
    }
}

/// The view rectangle and the units' panel shift (§1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ViewRect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
    /// `GeneralPlayAreaCameraShiftX` (`0x007A5214`).
    pub shift_x: i32,
}

impl ViewRect {
    /// The §1 table for frame `size` and open mode `mode`.
    pub fn new(size: FrameSize, mode: OpenMode) -> Self {
        let quarter = size.width / 4;
        let shift_x = match mode.0 {
            1 => -quarter,
            2 => quarter,
            _ => 0,
        };
        ViewRect {
            left: shift_x,
            top: 0,
            right: size.width + shift_x,
            bottom: size.play_height(),
            shift_x,
        }
    }
}

/// A position in client pixels (§2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ClientPos {
    pub x: i32,
    pub y: i32,
}

/// A unit position as the client keeps it (§2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UnitPosition {
    /// A moving unit (players, monsters, missiles): 16.16 fixed subtiles,
    /// unsigned.
    Moving { x16: u32, y16: u32 },
    /// A static unit (objects, items, tiles; unit types 2, 4, 5): integer
    /// subtiles.
    Static { sx: i32, sy: i32 },
}

impl UnitPosition {
    /// The unit's client pixel position (§2).
    pub fn client(&self) -> ClientPos {
        match *self {
            UnitPosition::Moving { x16, y16 } => moving_to_client(x16, y16),
            UnitPosition::Static { sx, sy } => static_to_client(sx, sy),
        }
    }
}

/// §2, moving units: `a = x16 >> 11`, `b = y16 >> 11` (logical), then
/// `px = (a − b) >> 1`, `py = (a + b) >> 2` (arithmetic: floor, not the
/// truncating `/` of D2MOO's 1.10f helper).
pub fn moving_to_client(x16: u32, y16: u32) -> ClientPos {
    // < 2^21 each, so the sums fit an i32.
    let a = (x16 >> 11) as i32;
    let b = (y16 >> 11) as i32;
    ClientPos {
        x: (a - b) >> 1,
        y: (a + b) >> 2,
    }
}

/// §2, static units: `px = (sx − sy) × 16`, `py = (sx + sy) × 8`.
pub fn static_to_client(sx: i32, sy: i32) -> ClientPos {
    ClientPos {
        x: (sx - sy) * 16,
        y: (sx + sy) * 8,
    }
}

/// §2, tiles: the client origin `(sx, sy)` of cell `(tx, ty)`.
pub fn cell_origin(tx: i32, ty: i32) -> ClientPos {
    ClientPos {
        x: (tx - ty) * CELL_HALF_WIDTH,
        y: (tx + ty) * CELL_HALF_HEIGHT,
    }
}

/// §2: the 1.14d tile entry `(e0, e1) = (sx − 80, sy + 80)`.
pub fn tile_entry(tx: i32, ty: i32) -> (i32, i32) {
    let c = cell_origin(tx, ty);
    (c.x - CELL_HALF_WIDTH, c.y + 2 * CELL_HALF_HEIGHT)
}

/// Which tile list a tile is drawn from (§6); which orientations go in
/// which list is `draw-order.md`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TileList {
    Floor,
    Wall,
    /// `roof_height`: the DT1 tile header field at `0x04`.
    Roof {
        roof_height: i32,
    },
}

/// The camera of one drawn frame (§3): computed once, from the local
/// player's client position, the open mode and the frame's shake offsets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Camera {
    pub size: FrameSize,
    pub view: ViewRect,
    /// Tile origin `(cx_t, cy_t)` (`view +0x24/+0x28`).
    pub tile: ClientPos,
    /// Unit origin `(cx_u, cy_u)` (`0x007A520C` / `0x007A5208`).
    pub unit: ClientPos,
}

impl Camera {
    /// §3. `shake` is `(dx, dy)` of §8, `(0, 0)` when no shake runs.
    pub fn new(size: FrameSize, mode: OpenMode, player: ClientPos, shake: (i32, i32)) -> Self {
        let view = ViewRect::new(size, mode);
        let (dx, dy) = shake;
        Camera {
            size,
            view,
            tile: ClientPos {
                x: player.x - (view.right - view.left) / 2 + dx,
                y: player.y - (view.bottom - view.top) / 2 + dy,
            },
            unit: ClientPos {
                x: player.x - size.width / 2 + dx,
                y: player.y - size.height / 2 + 16 + dy,
            },
        }
    }

    /// §4: the draw position (X, Y) of a unit at client `at` with the
    /// per-unit extra offsets `(ox, oy)` (`unit-composite.md`).
    pub fn unit_draw(&self, at: ClientPos, extra: (i32, i32)) -> (i32, i32) {
        (
            at.x + extra.0 - self.unit.x + self.view.shift_x,
            at.y + extra.1 - self.unit.y + 8,
        )
    }

    /// §6: the (X, Y) handed to the drawer for cell `(tx, ty)` in `list`.
    pub fn tile_handed(&self, list: TileList, tx: i32, ty: i32) -> (i32, i32) {
        self.handed_at(list, cell_origin(tx, ty))
    }

    /// §6 for a cell whose client origin is `s = (sx, sy)`.
    pub fn handed_at(&self, list: TileList, s: ClientPos) -> (i32, i32) {
        let (cx, cy) = (self.tile.x, self.tile.y);
        match list {
            TileList::Floor => (s.x - cx, s.y - cy),
            TileList::Wall => (
                s.x - CELL_HALF_WIDTH - cx + self.view.left,
                s.y + 2 * CELL_HALF_HEIGHT - cy + self.view.top,
            ),
            TileList::Roof { roof_height } => (s.x - cx, s.y - roof_height - cy),
        }
    }

    /// §5, §6: where block pixel `(0, 0)` of a tile handed `(x, y)` lands.
    /// The floor drawer (also used for roofs) moves X by −80 and by the
    /// panel shift `left`; the wall drawer takes (X, Y) as is.
    pub fn block_origin(&self, list: TileList, handed: (i32, i32)) -> (i32, i32) {
        match list {
            TileList::Floor | TileList::Roof { .. } => {
                (handed.0 - CELL_HALF_WIDTH + self.view.left, handed.1)
            }
            TileList::Wall => handed,
        }
    }

    /// §7: whether a floor or roof handed `(x, y)` is drawn: inside the
    /// view clip rectangle `[−80, W + 80) × [−80, H − 47)`.
    pub fn floor_roof_visible(&self, handed: (i32, i32)) -> bool {
        let (x, y) = handed;
        (-80..self.size.width + 80).contains(&x) && (-80..self.size.height - 47).contains(&y)
    }

    /// §7: whether a wall block at screen `(x, y)` (its top-left) is drawn:
    /// x in `[−32, W)` (modes 0/3), `[−32, W − W / 2)` (mode 1),
    /// `[W / 2 − 32, W)` (mode 2); y in `[−32, H + 32)`.
    pub fn wall_block_visible(&self, x: i32, y: i32) -> bool {
        let w = self.size.width;
        let xs = match self.view.shift_x {
            s if s < 0 => -32..w - w / 2,
            s if s > 0 => w / 2 - 32..w,
            _ => -32..w,
        };
        xs.contains(&x) && (-32..self.size.height + 32).contains(&y)
    }
}

/// A running screen shake (§8, started by `0x00476A80`): peak `A`,
/// attack `t1`, sustain `t2`, release `t3`, in milliseconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Shake {
    pub peak: u32,
    pub attack: u32,
    pub sustain: u32,
    pub release: u32,
}

impl Shake {
    /// `None` when `t2 = 0`: the start call is ignored (§8).
    pub fn start(peak: u32, attack: u32, sustain: u32, release: u32) -> Option<Self> {
        (sustain != 0).then_some(Shake {
            peak,
            attack,
            sustain,
            release,
        })
    }

    /// The amplitude `a` at `t` ms after the start (§8 table), or `None`
    /// once `t > t1 + t2 + t3` (the shake has ended; offsets 0). Every
    /// value is unsigned 32-bit as in the original: sums and the product
    /// keep their low 32 bits (`imul`), comparisons and the division are
    /// unsigned. A zero divisor is never reached: the attack row needs
    /// `t < t1`, and with `t3 = 0` the release row (only `t = t1 + t2`)
    /// gives `a = 0`.
    pub fn amplitude(&self, t: u32) -> Option<u32> {
        let (a, t1, t2, t3) = (self.peak, self.attack, self.sustain, self.release);
        let end = t1.wrapping_add(t2).wrapping_add(t3);
        if t > end {
            return None;
        }
        Some(if t < t1 {
            a.wrapping_mul(t) / t1
        } else if t < t1.wrapping_add(t2) {
            a
        } else {
            // t3 = 0: only t = t1 + t2 reaches this row, a = 0.
            a.wrapping_mul(end - t).checked_div(t3).unwrap_or(0)
        })
    }

    /// §9: the d2rs envelope time of the frame `ticks` client ticks after
    /// the start (`t = 40 × ticks`).
    pub fn time_of(ticks: u32) -> u32 {
        TICK_MS.wrapping_mul(ticks)
    }
}

/// §8: the frame's `(dx, dy)` for amplitude `a`, drawing from the local
/// player unit's seed: `dx = roll_range(−a, 2a)`, then `dy` the same
/// (helper `0x00472280`). `a = 0` draws nothing and gives `(0, 0)`.
pub fn shake_offsets(a: u32, seed: &mut Seed) -> (i32, i32) {
    if a == 0 {
        return (0, 0);
    }
    let a = a as i32;
    let n = a.wrapping_mul(2);
    let dx = seed.roll_range(a.wrapping_neg(), n);
    let dy = seed.roll_range(a.wrapping_neg(), n);
    (dx, dy)
}
