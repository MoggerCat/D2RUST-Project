// Spec: specs/client/render-pipeline.md (A2–A5)
// Spec: specs/render/composition.md (§5 one pixel write, §6)
//! Draw items and their inputs: frames, the map table, shade chains, blend
//! ops.

use super::order::DrawKey;
use super::{Rect, SceneError, MAX_SHADE};

/// Identifies one decoded index frame. The asset side (C3, `IndexFrame` and
/// the atlas) owns the numbering; the atlas slot is looked up from it by the
/// GPU compositor, never stored here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FrameId(pub u32);

/// A borrowed 8-bit index image: row-major, top row first, index 0 =
/// transparent (§A2). The pixel count is checked on construction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameView<'a> {
    width: u32,
    height: u32,
    pixels: &'a [u8],
}

impl<'a> FrameView<'a> {
    pub fn new(width: u32, height: u32, pixels: &'a [u8]) -> Result<Self, SceneError> {
        if pixels.len() as u64 != u64::from(width) * u64::from(height) {
            return Err(SceneError::FrameSize {
                width,
                height,
                len: pixels.len(),
            });
        }
        Ok(FrameView {
            width,
            height,
            pixels,
        })
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn pixels(&self) -> &'a [u8] {
        self.pixels
    }

    /// Index at frame coordinates; the caller keeps them in range.
    fn at(&self, x: u32, y: u32) -> u8 {
        self.pixels[y as usize * self.width as usize + x as usize]
    }
}

/// Where the compositor reads frames from. C3's frame store implements
/// this; a frame that is not resident is [`SceneError::FrameMissing`]
/// (spec: "Edge cases", never a skipped draw).
pub trait FrameSource {
    fn frame(&self, id: FrameId) -> Result<FrameView<'_>, SceneError>;
}

/// An owned index image, for tests and synthetic cases. Offsets are not
/// part of it: a [`DrawItem`] position is already the image's top-left.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrameImage {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

/// A slice of images is a frame source: `FrameId(n)` is element `n`.
impl FrameSource for [FrameImage] {
    fn frame(&self, id: FrameId) -> Result<FrameView<'_>, SceneError> {
        let img = self
            .get(id.0 as usize)
            .ok_or(SceneError::FrameMissing(id))?;
        FrameView::new(img.width, img.height, &img.pixels)
    }
}

impl FrameSource for Vec<FrameImage> {
    fn frame(&self, id: FrameId) -> Result<FrameView<'_>, SceneError> {
        self.as_slice().frame(id)
    }
}

/// A row of the [`MapTable`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MapId(pub u32);

/// Every index map of the frame, one row of 256 bytes each: PL2 maps and
/// the rows of 256×256 blend tables alike (§A4). The GPU uploads the same
/// rows as one storage buffer, so a `MapId` means the same on both sides.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MapTable {
    rows: Vec<[u8; 256]>,
}

impl MapTable {
    pub fn new() -> Self {
        Self::default()
    }

    /// Appends one row and returns its id.
    pub fn push(&mut self, row: [u8; 256]) -> MapId {
        let id = MapId(self.rows.len() as u32);
        self.rows.push(row);
        id
    }

    /// Appends a 256×256 blend table and returns the base id for
    /// [`BlendOp::IndexTable`]. The table is `table[dest][src]`: row =
    /// destination index, column = source index, the layout of the PL2
    /// tables as 1.14d reads them (`composition.md` §5), so a PL2 table is
    /// pushed unchanged.
    pub fn push_table(&mut self, table: &[[u8; 256]; 256]) -> MapId {
        let id = MapId(self.rows.len() as u32);
        self.rows.extend_from_slice(table);
        id
    }

    pub fn get(&self, id: MapId) -> Option<&[u8; 256]> {
        self.rows.get(id.0 as usize)
    }

    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// All rows in id order (what the GPU uploads).
    pub fn rows(&self) -> &[[u8; 256]] {
        &self.rows
    }

    fn row(&self, id: MapId) -> &[u8; 256] {
        &self.rows[id.0 as usize]
    }
}

/// Up to four maps applied in order to a non-zero source index (§A4):
/// `i' = m3[m2[m1[m0[i]]]]`, unused slots skipped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ShadeChain {
    maps: [MapId; MAX_SHADE],
    len: u8,
}

impl ShadeChain {
    /// The empty chain: the source index is used unchanged.
    pub const EMPTY: ShadeChain = ShadeChain {
        maps: [MapId(0); MAX_SHADE],
        len: 0,
    };

    pub fn new(maps: &[MapId]) -> Result<Self, SceneError> {
        if maps.len() > MAX_SHADE {
            return Err(SceneError::ShadeChainTooLong { len: maps.len() });
        }
        let mut chain = Self::EMPTY;
        chain.maps[..maps.len()].copy_from_slice(maps);
        chain.len = maps.len() as u8;
        Ok(chain)
    }

    pub fn maps(&self) -> &[MapId] {
        &self.maps[..usize::from(self.len)]
    }

    /// Applies the chain. Index 0 is tested by the caller before this.
    /// TODO(spec: render/shading.md): whether a mapped result of 0 is
    /// transparent (§B3). Until then the result is used as is: a mapped 0
    /// draws index 0.
    pub(super) fn apply(&self, maps: &MapTable, index: u8) -> u8 {
        self.maps()
            .iter()
            .fold(index, |i, &m| maps.row(m)[usize::from(i)])
    }
}

impl Default for ShadeChain {
    fn default() -> Self {
        Self::EMPTY
    }
}

/// How a shaded source index combines with the destination (§A5). The
/// framebuffer is indexed (u8 per pixel, palette at present): 1.14d's
/// reference renderer composes in the index domain, so `Rgb` is not an op
/// (`composition.md` §1, §6).
///
/// One pixel write of 1.14d (`composition.md` §5) is a shade chain then a
/// blend op; [`PixelTables::ops`] builds both from the draw's `P`, `L`
/// and `T` (with `L` and `T` the remap `P` is dropped). TODO(spec:
/// render/blend-modes.md): which op and table each draw uses (§B5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BlendOp {
    /// `dest = src` (no `T`: `d' = L[P[s]]`).
    Opaque,
    /// `dest = map[base + dest][src]`: a 256×256 table stored as rows
    /// `base..base + 256` of the map table, row = destination, column =
    /// source (`T[256 × d + s]`).
    IndexTable(MapId),
}

impl BlendOp {
    pub(super) fn apply(&self, maps: &MapTable, src: u8, dest: u8) -> u8 {
        match *self {
            BlendOp::Opaque => src,
            BlendOp::IndexTable(base) => {
                maps.row(MapId(base.0 + u32::from(dest)))[usize::from(src)]
            }
        }
    }
}

/// The up to three tables 1.14d's row drawer takes for one draw
/// (`composition.md` §5): remap `P`, light `L` and blend table `T` (the
/// base id of a 256×256 table pushed with [`MapTable::push_table`]).
/// Which draw passes which tables is `blend-modes.md`'s.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct PixelTables {
    pub remap: Option<MapId>,
    pub light: Option<MapId>,
    pub blend: Option<MapId>,
}

impl PixelTables {
    /// The shade chain and blend op of §5:
    /// - no `T`: `d' = L[P[s]]` (chain `P`, `L`; absent steps skipped);
    /// - `T`, no `L`: `d' = T[256 × d + P[s]]`;
    /// - `T` and `L`: `d' = T[256 × d + L[s]]`: `P` is not applied, whether
    ///   or not the draw passed one (the dispatcher picks the `L` routine).
    pub fn ops(&self) -> (ShadeChain, BlendOp) {
        let chain = |maps: &[Option<MapId>]| {
            let mut c = ShadeChain::EMPTY;
            for m in maps.iter().flatten() {
                c.maps[usize::from(c.len)] = *m;
                c.len += 1;
            }
            c
        };
        match (self.light, self.blend) {
            (_, None) => (chain(&[self.remap, self.light]), BlendOp::Opaque),
            (None, Some(t)) => (chain(&[self.remap]), BlendOp::IndexTable(t)),
            (Some(l), Some(t)) => (chain(&[Some(l)]), BlendOp::IndexTable(t)),
        }
    }
}

/// Debug label of an item; never read by the compositor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ItemTag {
    #[default]
    None,
    /// A unit by GUID.
    Unit(u32),
    /// A map tile by cell.
    Tile { x: i32, y: i32 },
    /// A UI element by id.
    Ui(u32),
}

/// One sprite draw (§A3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DrawItem {
    pub frame: FrameId,
    /// Screen position of the image's top-left, integer pixels. Turning
    /// format offsets into this is the builder's job (§B1).
    pub x: i32,
    pub y: i32,
    /// Reserved (§A3). TODO(spec: render/sprite-placement.md,
    /// render/unit-composite.md): set only when an owner spec requires a
    /// mirrored draw; until then `true` is rejected ([`SceneError::FlipX`]).
    pub flip_x: bool,
    /// Pixels outside this screen rectangle are not drawn.
    pub clip: Rect,
    pub shade: ShadeChain,
    pub blend: BlendOp,
    pub key: DrawKey,
    pub tag: ItemTag,
}

impl DrawItem {
    /// An opaque, unshaded, unflipped item clipped to the frame, key 0.
    pub fn new(frame: FrameId, x: i32, y: i32) -> Self {
        DrawItem {
            frame,
            x,
            y,
            flip_x: false,
            clip: Rect::FRAME,
            shade: ShadeChain::EMPTY,
            blend: BlendOp::Opaque,
            key: DrawKey::default(),
            tag: ItemTag::None,
        }
    }

    /// Checks everything the compositor will index, so composing cannot
    /// fail or panic afterwards. Returns the frame and the screen rectangle
    /// the item may touch inside `view` (`None`: nothing to draw).
    pub(super) fn resolve<'f, F: FrameSource + ?Sized>(
        &self,
        frames: &'f F,
        maps: &MapTable,
        view: &Rect,
    ) -> Result<(FrameView<'f>, Option<Rect>), SceneError> {
        if self.flip_x {
            return Err(SceneError::FlipX);
        }
        let frame = frames.frame(self.frame)?;
        for &m in self.shade.maps() {
            maps.get(m).ok_or(SceneError::MapMissing(m))?;
        }
        if let BlendOp::IndexTable(base) = self.blend {
            if u64::from(base.0) + 256 > maps.len() as u64 {
                return Err(SceneError::BlendTable(base));
            }
        }
        let image = Rect::new(self.x, self.y, frame.width(), frame.height());
        let area = image.intersect(&self.clip).and_then(|r| r.intersect(view));
        Ok((frame, area))
    }

    /// The shaded, blended value of screen pixel `(sx, sy)` over `dest`, or
    /// `dest` unchanged where the frame is transparent. `(sx, sy)` lies in
    /// the area returned by [`DrawItem::resolve`].
    pub(super) fn pixel(
        &self,
        frame: &FrameView<'_>,
        maps: &MapTable,
        sx: i64,
        sy: i64,
        dest: u8,
    ) -> u8 {
        let fx = (sx - i64::from(self.x)) as u32;
        let fy = (sy - i64::from(self.y)) as u32;
        let src = frame.at(fx, fy);
        if src == 0 {
            return dest;
        }
        self.blend.apply(maps, self.shade.apply(maps, src), dest)
    }
}
