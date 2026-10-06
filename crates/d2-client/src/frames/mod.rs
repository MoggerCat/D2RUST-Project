// Spec: specs/client/render-pipeline.md
//! Indexed frames and per-direction frame sets (§A2; derived assets in
//! `specs/client/assets.md` §A3), and the atlas they are packed into.
//!
//! Plain Rust: no Bevy types outside [`upload`], the page upload edge.
//! Frames carry no color and no screen position: offsets are the file
//! format's own values, unchanged; how they become a screen position is
//! `render/sprite-placement.md` (render-pipeline §B1), not decided here.

pub mod atlas;
pub mod upload;

#[cfg(test)]
mod tests;

use std::fmt;

use d2_formats::dc6::Dc6;
use d2_formats::dcc::Dcc;
use d2_formats::dt1::Dt1;

pub use atlas::{Atlas, AtlasError, AtlasSlot, CheckReport, GUTTER, PAGE_SIZE};

/// One decoded frame: 8-bit palette indices, row-major, top row first,
/// index 0 = transparent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexFrame {
    pub width: u32,
    pub height: u32,
    /// The format's own offsets, unchanged: DC6 `offset_x/offset_y`, DCC
    /// frame box top-left `(x_min, y_min)`, DT1 tile image `(x0, y0)`.
    // TODO(spec: render/sprite-placement.md): offsets → screen pixel (§B1).
    pub x_off: i32,
    pub y_off: i32,
    pub pixels: Vec<u8>,
}

impl IndexFrame {
    /// A frame whose pixel count must equal `width × height`.
    pub fn new(
        width: u32,
        height: u32,
        x_off: i32,
        y_off: i32,
        pixels: Vec<u8>,
    ) -> Result<Self, FrameError> {
        let expected = u64::from(width) * u64::from(height);
        if pixels.len() as u64 != expected {
            return Err(FrameError::PixelCount {
                width,
                height,
                len: pixels.len(),
            });
        }
        Ok(Self {
            width,
            height,
            x_off,
            y_off,
            pixels,
        })
    }

    /// No pixels at all (a 0-wide or 0-high frame).
    pub fn is_empty(&self) -> bool {
        self.width == 0 || self.height == 0
    }
}

/// Which part of a file a frame set holds (`assets.md` §A3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FramePart {
    /// One direction of a DCC or DC6.
    Dir(u8),
    /// One tile of a DT1, by its index in the file's tile list.
    Tile(u32),
}

/// Key of a derived frame set: a canonical archive path plus the part.
///
/// The path is the canonical form of `assets.md` §A1 (lowercase ASCII,
/// `/` separators, no leading `/`). Canonicalizing is the asset task's
/// job (C1); this type only refuses a path not already canonical.
/// Ordered (`BTreeMap` keys for deterministic eviction, §A5).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FrameSetKey {
    path: String,
    part: FramePart,
}

impl FrameSetKey {
    pub fn new(path: impl Into<String>, part: FramePart) -> Result<Self, FrameError> {
        let path = path.into();
        let canonical = !path.is_empty()
            && !path.starts_with('/')
            && path
                .bytes()
                .all(|b| b.is_ascii() && !b.is_ascii_uppercase() && b != b'\\');
        if !canonical {
            return Err(FrameError::NonCanonicalPath(path));
        }
        Ok(Self { path, part })
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn part(&self) -> FramePart {
        self.part
    }
}

/// The frames of one direction (DCC/DC6) or one tile (DT1), in file
/// order. A DT1 tile without blocks has no image (`map-preview.md` §Tile
/// images) and gives an empty set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrameSet {
    pub frames: Vec<IndexFrame>,
}

impl FrameSet {
    /// Budget size (`assets.md` §A5): `Σ width × height` plus a fixed
    /// header per frame.
    pub fn byte_size(&self) -> u64 {
        let header = std::mem::size_of::<IndexFrame>() as u64;
        self.frames
            .iter()
            .map(|f| f.pixels.len() as u64 + header)
            .sum()
    }

    /// Direction `dir` of a DCC; frames keep their DCC frame boxes.
    pub fn from_dcc(dcc: &Dcc, dir: u8) -> Result<Self, FrameError> {
        let d = dcc
            .directions
            .get(usize::from(dir))
            .ok_or(FrameError::NoPart {
                part: FramePart::Dir(dir),
                count: dcc.directions.len(),
            })?;
        let frames = d
            .frames
            .iter()
            .map(|f| IndexFrame::new(f.width, f.height, f.x_min, f.y_min, f.pixels.clone()))
            .collect::<Result<_, _>>()?;
        Ok(Self { frames })
    }

    /// Direction `dir` of a DC6; frames keep `offset_x/offset_y`.
    pub fn from_dc6(dc6: &Dc6, dir: u8) -> Result<Self, FrameError> {
        let dirs = dc6.header.directions as usize;
        let per = dc6.header.frames_per_direction as usize;
        if usize::from(dir) >= dirs {
            return Err(FrameError::NoPart {
                part: FramePart::Dir(dir),
                count: dirs,
            });
        }
        let frames = (0..per)
            .map(|i| {
                let f = dc6
                    .frame(usize::from(dir), i)
                    .ok_or(FrameError::MissingFrame { dir, frame: i })?;
                IndexFrame::new(f.width, f.height, f.offset_x, f.offset_y, f.pixels.clone())
            })
            .collect::<Result<_, _>>()?;
        Ok(Self { frames })
    }

    /// Tile `tile` of a DT1: its blocks assembled into one image by the
    /// `map-preview.md` rule (`crate::map::tiles::assemble`).
    pub fn from_dt1(dt1: &Dt1, tile: u32) -> Result<Self, FrameError> {
        let t = dt1.tiles.get(tile as usize).ok_or(FrameError::NoPart {
            part: FramePart::Tile(tile),
            count: dt1.tiles.len(),
        })?;
        let frames = match crate::map::tiles::assemble(t) {
            Some(img) => vec![IndexFrame::new(
                img.width, img.height, img.x0, img.y0, img.pixels,
            )?],
            None => Vec::new(),
        };
        Ok(Self { frames })
    }
}

/// A file parsed by `d2-formats`, ready to give frame sets.
#[derive(Debug, Clone, Copy)]
pub enum FrameSource<'a> {
    Dcc(&'a Dcc),
    Dc6(&'a Dc6),
    Dt1(&'a Dt1),
}

impl FrameSource<'_> {
    /// The frame set `part` of this file. A direction of a DT1 or a tile
    /// of a DCC/DC6 is an error (M07), never a guess.
    pub fn frame_set(&self, part: FramePart) -> Result<FrameSet, FrameError> {
        match (*self, part) {
            (FrameSource::Dcc(f), FramePart::Dir(d)) => FrameSet::from_dcc(f, d),
            (FrameSource::Dc6(f), FramePart::Dir(d)) => FrameSet::from_dc6(f, d),
            (FrameSource::Dt1(f), FramePart::Tile(t)) => FrameSet::from_dt1(f, t),
            (_, part) => Err(FrameError::WrongPartKind(part)),
        }
    }

    /// Number of parts (directions or tiles) the file has.
    pub fn part_count(&self) -> usize {
        match *self {
            FrameSource::Dcc(f) => f.directions.len(),
            FrameSource::Dc6(f) => f.header.directions as usize,
            FrameSource::Dt1(f) => f.tiles.len(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FrameError {
    #[error("frame {width}x{height} has {len} pixels")]
    PixelCount { width: u32, height: u32, len: usize },
    #[error("{part} out of range: the file has {count}")]
    NoPart { part: FramePart, count: usize },
    #[error("DC6 direction {dir} has no frame {frame}")]
    MissingFrame { dir: u8, frame: usize },
    #[error("{0} does not apply to this file type")]
    WrongPartKind(FramePart),
    #[error("path {0:?} is not canonical (lowercase ASCII, '/' separators, no leading '/')")]
    NonCanonicalPath(String),
}

impl fmt::Display for FramePart {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FramePart::Dir(d) => write!(f, "direction {d}"),
            FramePart::Tile(t) => write!(f, "tile {t}"),
        }
    }
}
