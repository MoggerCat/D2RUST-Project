// Spec: specs/render/draw-order-2.md (§12 level backgrounds, l2 r1–r3), specs/render/blend-modes.md (§4 cel modes), specs/render/sprite-placement.md (§2)
//! Pass 1 of the world draw in the play preview (`draw-order.md` §1 row
//! 1): the level background of Arreat Summit (level 120) through
//! [`Backgrounds::pass1`], its cels (`data\global\ui\summit01`,
//! `cloud01`) read from the user's archives on first use and drawn at
//! their screen positions with their light and draw mode through the
//! act's blend tables ([`cel_ops`]).
//!
//! Level 74 (the Arcane Sanctuary stars) stays undrawn: §12 r3 names no
//! initial star tick `last` (`BackgroundError::NoStarLast`), and the
//! draw order refuses the level (`order_grid`, Open 1).
//!
//! `d2rs-own, unverified`:
//! - PROVISIONAL (REC-420): the summit's first-use seed is
//!   `init_low(time_value(0))` of the host clock (§12: "seed := time
//!   value"; `sim/rng.md` §5.5 names neither the argument of
//!   `time_value` nor the seed form); [`summit_seed_now`]. The clouds'
//!   placement is time-seeded in 1.14d too, so only a capture that
//!   records `[0x00712C50]` can compare it;
//! - the cel light byte is the low byte of the light argument (−1 →
//!   0xFF unlit, `0xDDDDDDDD` → 0xDD; `shading.md` §3 takes one byte);
//! - without the act's shade tables a mode-5 cel draws opaque and a
//!   mode-3 cel is not drawn;
//! - a file no archive holds is logged once and its cels are not drawn.

use std::collections::BTreeMap;
use std::sync::Arc;

use d2_sim::rng::Seed;

use crate::assets::path::{CanonicalPath, FileSource};
use crate::bridge::world::ClientWorld;
use crate::frames::{FramePart, FrameSet, FrameSetKey};
use crate::rules::blend::cel_ops;
use crate::rules::draw_order::background::{
    BackgroundDraw, BackgroundFrame, Backgrounds, ARREAT_SUMMIT,
};
use crate::rules::placement::draw_position;
use crate::scene::{BlendOp, DrawItem, DrawKey, ItemTag, ShadeChain};

use super::{ViewAssets, WorldFrame};

/// Draw mode 5: opaque (`blend-modes.md` §4).
const MODE_OPAQUE: u8 = 5;

/// The level backgrounds' layer of the world view: the kept state of
/// §12 (first-call flags, clouds), the file source and the cel files
/// loaded so far.
pub struct BackgroundView {
    source: Arc<dyn FileSource>,
    state: Backgrounds,
    /// The recorded / host seed of the summit's first use.
    summit_seed: Option<Seed>,
    files: BTreeMap<&'static str, Option<FrameSetKey>>,
}

impl std::fmt::Debug for BackgroundView {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BackgroundView")
            .field("state", &self.state)
            .field("summit_seed", &self.summit_seed)
            .finish_non_exhaustive()
    }
}

/// PROVISIONAL (REC-420, module doc): the summit's first-use seed from
/// the host clock: `init_low(time_value(0))` with `time()` the Unix
/// seconds and `GetTickCount()` the milliseconds of the same clock.
pub fn summit_seed_now() -> Seed {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let base = (now.as_secs() as u32).wrapping_add(now.as_millis() as u32);
    // `time_value(v)` adds `v` to `time() + GetTickCount()`.
    Seed::init_low(d2_sim::rng::time_value(base))
}

impl BackgroundView {
    /// A layer over `source`; `summit_seed` is the seed of level 120's
    /// first use (`None`: the background is not drawn, §12 needs it).
    pub fn new(source: Arc<dyn FileSource>, summit_seed: Option<Seed>) -> Self {
        BackgroundView {
            source,
            state: Backgrounds::default(),
            summit_seed,
            files: BTreeMap::new(),
        }
    }

    /// The kept §12 state.
    pub fn state(&self) -> &Backgrounds {
        &self.state
    }

    fn file_key(
        &mut self,
        file: &'static str,
        assets: &mut ViewAssets,
        log: &mut Vec<String>,
    ) -> Option<FrameSetKey> {
        if let Some(k) = self.files.get(file) {
            return k.clone();
        }
        let loaded = load(self.source.as_ref(), file, assets);
        if let Err(e) = &loaded {
            log.push(format!("level background (d2rs-own, unverified): {e}"));
        }
        let key = loaded.ok();
        self.files.insert(file, key.clone());
        key
    }

    /// Pass 1 of this frame (`draw-order.md` §1 row 1): when the local
    /// player's level is 120 and the frame has a camera, the §12 draws
    /// added to `frame` (re-sorted by key) at their screen positions.
    /// `player_x` is the local player's client x (the frame anchor's).
    /// Open mode 3 draws no world (§1). Never fails the frame; returns
    /// log lines.
    pub fn add_to_frame(
        &mut self,
        world: &ClientWorld,
        open_mode: u8,
        player_x: i32,
        assets: &mut ViewAssets,
        frame: &mut WorldFrame,
    ) -> Vec<String> {
        let mut log = Vec::new();
        let level = world.player_level().map_or(0, u32::from);
        if level != ARREAT_SUMMIT || open_mode == 3 {
            return log;
        }
        let Some(cam) = frame.camera else {
            return log;
        };
        let f = BackgroundFrame {
            level,
            w: cam.size.width,
            h: cam.size.height,
            resolution_mode: cam.size.resolution_mode(),
            // Read by level 74 only.
            now: 0,
            exiting: world.exit_requested,
            player_x,
            stars_seed: None,
            summit_seed: self.summit_seed,
            stars_last: None,
            palette: None,
        };
        let items = match self.state.pass1(&f) {
            Ok(items) => items,
            Err(e) => {
                log.push(format!("level background: {e}"));
                return log;
            }
        };
        let mut added = false;
        for it in items {
            let BackgroundDraw::Cel {
                file,
                frame: index,
                x,
                y,
                light,
                draw_mode,
            } = it.draw
            else {
                continue;
            };
            let Some(set) = self.file_key(file, assets, &mut log) else {
                continue;
            };
            let index = index as usize;
            let (Ok(id), Ok(image)) = (assets.id(&set, index), assets.frame(&set, index)) else {
                continue;
            };
            let (shade, blend) = match &assets.shades {
                Some(t) => cel_ops(t, draw_mode, None, light as u8),
                None if draw_mode == MODE_OPAQUE => (ShadeChain::EMPTY, BlendOp::Opaque),
                None => continue,
            };
            let (px, py) = draw_position(image, x, y);
            let Ok(key) = DrawKey::new(it.key.pass, it.key.major, it.key.minor, 0) else {
                continue;
            };
            let mut item = DrawItem::new(id, px, py);
            item.key = key;
            item.shade = shade;
            item.blend = blend;
            item.tag = ItemTag::None;
            frame.items.push(item);
            added = true;
        }
        if added {
            crate::scene::order(&mut frame.items);
        }
        log
    }
}

/// Reads `file` (a §12 cel file, no extension) as a DC6 into the frame
/// store, direction 0.
fn load(
    source: &dyn FileSource,
    file: &str,
    assets: &mut ViewAssets,
) -> Result<FrameSetKey, String> {
    let path = format!("{file}.dc6");
    let canon = CanonicalPath::new(&path).map_err(|e| format!("{path}: {e}"))?;
    let key = FrameSetKey::new(canon.as_str(), FramePart::Dir(0)).map_err(|e| e.to_string())?;
    if assets.frames.contains(&key) {
        return Ok(key);
    }
    let dc6 = crate::assets::path::read_dc6(source, &path)
        .ok_or_else(|| format!("{path}: in no archive"))?
        .map_err(|e| format!("{path}: {e}"))?;
    let set = FrameSet::from_dc6(&dc6, 0).map_err(|e| format!("{path}: {e}"))?;
    assets
        .frames
        .insert(key.clone(), set)
        .map_err(|e| e.to_string())?;
    Ok(key)
}

#[cfg(test)]
#[path = "background_view_tests.rs"]
mod tests;
