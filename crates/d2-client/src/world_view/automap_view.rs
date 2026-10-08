// Spec: specs/ui/automap.md (§8 r5, §10, §11), specs/render/sprite-placement.md (§2)
//! The automap in the play preview's world view: the session's draw pass
//! ([`crate::ui::automap::Automap::draw_pass`]) turned into frame items.
//! The cel files (`DATA\GLOBAL\UI\AutoMap\*.dc6`, §8 r5) are read from the
//! user's archives on demand; the marker lines are drawn as one-pixel
//! cels of the line's palette index.
//!
//! Each cel draws in its §10 r4 mode `m` through the act's blend tables
//! (`blend-modes.md` §1: 0/1/2 → the 25/50/75 % alpha tables, 5 opaque),
//! unlit; the lines are opaque (`blend-modes.md` §8 r1). The pass is the
//! UI pass's step 3 (`ui/panels.md` §5 r3, `draw-order.md` OQ14): after
//! every world pass, before every panel draw.
//!
//! d2rs-own, unverified (decision D1; automap-0001, automap-0002):
//! - without the act's shade tables (no feed loaded them) a cel draws
//!   opaque, whatever its mode;
//! - the header and name texts (§11 r7, §13) are not drawn (no text path
//!   for them yet); only the cells and the unit markers;
//! - the local player is the only marker unit (the near rooms' unit
//!   lists are not turned into markers); the player's byte +0x18 is 0;
//! - a file no archive holds is logged once and its cels are not drawn.

use std::collections::BTreeMap;
use std::sync::Arc;

use crate::assets::path::{CanonicalPath, FileSource};
use crate::bridge::world::ClientWorld;
use crate::frames::{FramePart, FrameSet, FrameSetKey, IndexFrame};
use crate::rules::blend::cel_ops;
use crate::rules::camera::{ClientPos, FrameSize, OpenMode};
use crate::rules::placement::draw_position;
use crate::scene::order::pass;
use crate::scene::{BlendOp, DrawItem, DrawKey, ItemTag, Rect, ShadeChain};
use crate::ui::automap::draw::AutomapDraw;
use crate::ui::automap::header::HeaderFacts;
use crate::ui::automap::markers::{MarkerPalette, MarkerSubject, MarkerUnit};
use crate::ui::automap::options::{cel_paths, CelFile, MemoryStore};
use crate::ui::automap::session::AutomapSession;
use crate::ui::automap::{FrameFacts, PassInput};

use super::{ViewAssets, WorldFrame};

/// The light byte of an unlit cel draw (`shading.md` §3).
const UNLIT: u8 = 0xFF;

/// The frame set of the one-pixel line cels (not an archive path).
const PIXEL_PATH: &str = "d2rs/preview/automap-pixel";

/// The cel files and line pixels of the automap, and what was logged.
pub struct AutomapView {
    source: Arc<dyn FileSource>,
    expansion: bool,
    /// (file, mini) → its frame set; `None`: tried and missing.
    files: BTreeMap<(CelFile, bool), Option<FrameSetKey>>,
    /// Option store of the pass (the registry seam; in memory here).
    store: MemoryStore,
    /// Draws of the last frame, for tests and logs.
    pub last: Vec<AutomapDraw>,
}

impl AutomapView {
    pub fn new(source: Arc<dyn FileSource>, expansion: bool) -> Self {
        AutomapView {
            source,
            expansion,
            files: BTreeMap::new(),
            store: MemoryStore::default(),
            last: Vec::new(),
        }
    }

    fn file_key(
        &mut self,
        file: CelFile,
        mini: bool,
        assets: &mut ViewAssets,
        log: &mut Vec<String>,
    ) -> Option<FrameSetKey> {
        if let Some(k) = self.files.get(&(file, mini)) {
            return k.clone();
        }
        let loaded = self.load(file, mini, assets);
        if let Err(e) = &loaded {
            log.push(format!("automap (d2rs-own, unverified): {e}"));
        }
        let key = loaded.ok();
        self.files.insert((file, mini), key.clone());
        key
    }

    fn load(
        &self,
        file: CelFile,
        mini: bool,
        assets: &mut ViewAssets,
    ) -> Result<FrameSetKey, String> {
        let path = cel_paths(mini, self.expansion)[file as usize]
            .clone()
            .ok_or_else(|| format!("{file:?} has no file in this size"))?;
        let canon = CanonicalPath::new(&path).map_err(|e| format!("{path}: {e}"))?;
        let key = FrameSetKey::new(canon.as_str(), FramePart::Dir(0)).map_err(|e| e.to_string())?;
        if assets.frames.contains(&key) {
            return Ok(key);
        }
        let dc6 = crate::assets::path::read_dc6(self.source.as_ref(), &path)
            .ok_or_else(|| format!("{path}: in no archive"))?
            .map_err(|e| format!("{path}: {e}"))?;
        let set = FrameSet::from_dc6(&dc6, 0).map_err(|e| format!("{path}: {e}"))?;
        assets
            .frames
            .insert(key.clone(), set)
            .map_err(|e| e.to_string())?;
        Ok(key)
    }

    /// The pixel set: frame `i` is one pixel of palette index `i`.
    fn pixel_key(assets: &mut ViewAssets) -> Result<FrameSetKey, String> {
        let key = FrameSetKey::new(PIXEL_PATH, FramePart::Tile(0)).map_err(|e| e.to_string())?;
        if !assets.frames.contains(&key) {
            let frames = (0..=255u8)
                .map(|i| IndexFrame::new(1, 1, 0, 0, vec![i]).map_err(|e| e.to_string()))
                .collect::<Result<Vec<_>, _>>()?;
            assets
                .frames
                .insert(key.clone(), FrameSet { frames })
                .map_err(|e| e.to_string())?;
        }
        Ok(key)
    }

    /// Runs the draw pass of `session` for `world` and adds the draws to
    /// `frame` (re-sorted by key), under the frame's one camera
    /// (`frame.camera`, `seams/world-screen.md` §2.2) with the local
    /// player's marker at `player`, the frame's one player position
    /// (§2.4). Closed automap, open mode 3, no local player, no camera or
    /// no act: nothing. Never fails the frame; returns log lines.
    pub fn add_to_frame(
        &mut self,
        session: &mut AutomapSession,
        world: &ClientWorld,
        open_mode: OpenMode,
        player_at: ClientPos,
        assets: &mut ViewAssets,
        frame: &mut WorldFrame,
    ) -> Vec<String> {
        self.last.clear();
        let mut log = Vec::new();
        let Some(player) = world.local() else {
            return log;
        };
        let Some(cam) = frame.camera else {
            return log;
        };
        // §10 r1: state 0x0A open and open mode ≠ 3.
        if !session.open || world.act.is_none() || open_mode.get() == 3 {
            return log;
        }
        let at = player_at;
        let facts = FrameFacts {
            width: FrameSize::D2RS.width,
            height: FrameSize::D2RS.height,
            open_mode: open_mode.get(),
            mini_down: false,
            unit_origin: cam.unit,
        };
        // d2rs-own, unverified: the marker colour is the act palette's
        // nearest match of §11 r3 on the presented palette.
        let palette = MarkerPalette::new(|r, g, b| {
            crate::rules::shading::nearest(&assets.palette, r.into(), g.into(), b.into())
        });
        let markers = [MarkerUnit {
            subject: MarkerSubject::Player {
                local: true,
                mode: player.mode,
                party: -1,
                own_inventory: true,
                state_7: false,
                name: Vec::new(),
            },
            pos: at,
        }];
        let header = HeaderFacts::default();
        let strings = |_: u16| None;
        let input = PassInput {
            frame: facts,
            open: true,
            ready: true,
            player_byte_18: 0,
            markers: &markers,
            local_party: -1,
            player_gate: false,
            palette,
            roster: &[],
            local_act: 0,
            header: &header,
            strings: &strings,
        };
        let draws = match session.map.draw_pass(&input, &mut self.store) {
            Ok(d) => d,
            Err(e) => {
                log.push(format!("automap (d2rs-own, unverified): {e}"));
                return log;
            }
        };
        let mini = session.map.view.mini;
        let pixel = match Self::pixel_key(assets) {
            Ok(k) => k,
            Err(e) => {
                log.push(format!("automap (d2rs-own, unverified): {e}"));
                return log;
            }
        };
        let mut minor = 0u32;
        let mut push = |frame: &mut WorldFrame, mut d: DrawItem| {
            if let Ok(k) = DrawKey::new(pass::UI, pass::UI_AUTOMAP_MAJOR, minor, 0) {
                d.key = k;
                d.tag = ItemTag::None;
                minor += 1;
                frame.items.push(d);
            }
        };
        for d in &draws {
            match d {
                AutomapDraw::Cel {
                    file,
                    cel,
                    x,
                    y,
                    clip,
                    mode,
                } => {
                    let Some(key) = self.file_key(*file, mini, assets, &mut log) else {
                        continue;
                    };
                    let Ok(index) = usize::try_from(*cel) else {
                        continue;
                    };
                    let (Ok(id), Ok(image)) = (assets.id(&key, index), assets.frame(&key, index))
                    else {
                        continue;
                    };
                    let (px, py) = draw_position(image, *x, *y);
                    let mut item = DrawItem::new(id, px, py);
                    item.clip = rect(clip);
                    (item.shade, item.blend) = cel_blend(assets, *mode);
                    push(frame, item);
                }
                AutomapDraw::Line { from, to, color } => {
                    let Ok(id) = assets.id(&pixel, usize::from(*color)) else {
                        continue;
                    };
                    for (x, y) in line(*from, *to) {
                        let mut item = DrawItem::new(id, x, y);
                        item.blend = BlendOp::Opaque;
                        push(frame, item);
                    }
                }
                // Module doc: texts are not drawn.
                AutomapDraw::Text { .. } => {}
            }
        }
        crate::scene::order(&mut frame.items);
        self.last = draws;
        log
    }
}

/// The shade chain and blend op of an automap cel in draw mode `mode`
/// (§10 r4; `blend-modes.md` §1, §2), unlit (light byte 0xFF). Without the
/// act's shade tables: opaque (module doc).
fn cel_blend(assets: &ViewAssets, mode: u8) -> (ShadeChain, BlendOp) {
    match &assets.shades {
        Some(t) => cel_ops(t, mode, None, UNLIT),
        None => (ShadeChain::EMPTY, BlendOp::Opaque),
    }
}

fn rect(b: &crate::ui::automap::view::Bounds) -> Rect {
    let w = (b.right - b.left + 1).max(0) as u32;
    let h = (b.bottom - b.top + 1).max(0) as u32;
    Rect::new(b.left, b.top, w, h)
}

/// The pixels of a line (Bresenham, both ends included).
fn line(from: (i32, i32), to: (i32, i32)) -> Vec<(i32, i32)> {
    let (mut x, mut y) = from;
    let (dx, dy) = ((to.0 - x).abs(), -(to.1 - y).abs());
    let (sx, sy) = (if x < to.0 { 1 } else { -1 }, if y < to.1 { 1 } else { -1 });
    let mut err = dx + dy;
    let mut out = Vec::new();
    loop {
        out.push((x, y));
        if (x, y) == to {
            return out;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            err += dx;
            y += sy;
        }
    }
}

#[cfg(test)]
#[path = "automap_view_tests.rs"]
mod tests;
