// Spec: specs/render/draw-order-2.md (§12 level backgrounds, l2 r1–r3), specs/render/blend-modes.md (§4 cel modes), specs/render/sprite-placement.md (§2)
//! Pass 1 of the world draw in the play preview (`draw-order.md` §1 row
//! 1): the level background of Arreat Summit (level 120) through
//! [`Backgrounds::pass1`], its cels (`data\global\ui\summit01`,
//! `cloud01`) read from the user's archives on first use and drawn at
//! their screen positions with their light and draw mode through the
//! act's blend tables ([`cel_ops`]).
//!
//! Level 74 (the Arcane Sanctuary stars) is drawn too: the star tick
//! `last` is 0 before the first call (§12 l74 r3), the star seed is
//! `init_low(time_value(time() + GetTickCount() + shake start))`
//! (§12 l74 r1; [`stars_seed_at`]); the summit's is
//! `init_low(time_value(time() + 2 * GetTickCount()))`
//! ([`summit_seed_at`]). Both backgrounds initialise once per process.
//! Their layouts are time-seeded in 1.14d too, so only a capture that
//! records `[0x00712C4C]` / `[0x00712C50]` can compare them.
//!
//! The star lines have no scene item yet: level 74's state advances (so
//! the moves keep their ticks) but nothing is added to the frame.
//!
//! `d2rs-own, unverified`:
//! - the screen-shake start tick of the star seed is 0 until a caller
//!   sets it ([`BackgroundView::set_shake_start`]); the feed keeps shake
//!   starts as server ticks, not `GetTickCount` values;
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
    BackgroundDraw, BackgroundFrame, Backgrounds, ARCANE_SANCTUARY, ARREAT_SUMMIT,
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
    /// The recorded seed of the stars' first use (`None`: the host
    /// clock's, [`stars_seed_at`]).
    stars_seed: Option<Seed>,
    /// The last screen-shake start tick (`[0x007B8D10]`, 0 if none).
    shake_start: u32,
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

/// The host clock as `(time(), GetTickCount())`: the Unix seconds and the
/// milliseconds of the same clock.
pub fn host_clock() -> (u32, u32) {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    (now.as_secs() as u32, now.as_millis() as u32)
}

/// §12 l120 r1: the summit's seed, `init_low(time_value(x))` with x =
/// `time() + GetTickCount() + GetTickCount()` (the call's `v` is the tick
/// count).
pub fn summit_seed_at(time: u32, tick: u32) -> Seed {
    Seed::init_low(d2_sim::rng::time_value(
        time.wrapping_add(tick).wrapping_add(tick),
    ))
}

/// §12 l74 r1: the stars' seed, `init_low(time_value(time() +
/// GetTickCount() + shake_start))`.
pub fn stars_seed_at(time: u32, tick: u32, shake_start: u32) -> Seed {
    Seed::init_low(d2_sim::rng::time_value(
        time.wrapping_add(tick).wrapping_add(shake_start),
    ))
}

/// The summit's first-use seed from the host clock.
pub fn summit_seed_now() -> Seed {
    let (t, tick) = host_clock();
    summit_seed_at(t, tick)
}

impl BackgroundView {
    /// A layer over `source`; `summit_seed` is the seed of level 120's
    /// first use (`None`: the background is not drawn, §12 needs it).
    pub fn new(source: Arc<dyn FileSource>, summit_seed: Option<Seed>) -> Self {
        BackgroundView {
            source,
            state: Backgrounds::default(),
            summit_seed,
            stars_seed: None,
            shake_start: 0,
            files: BTreeMap::new(),
        }
    }

    /// Fixes the stars' first-use seed (a recorded one); by default the
    /// host clock's at first use.
    pub fn with_stars_seed(mut self, seed: Seed) -> Self {
        self.stars_seed = Some(seed);
        self
    }

    /// The last screen-shake start tick, read by the stars' first-use
    /// seed (`render/camera.md` §8; 0 when none started).
    pub fn set_shake_start(&mut self, tick: u32) {
        self.shake_start = tick;
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
        if !matches!(level, ARREAT_SUMMIT | ARCANE_SANCTUARY) || open_mode == 3 {
            return log;
        }
        let Some(cam) = frame.camera else {
            return log;
        };
        let (time, tick) = host_clock();
        let f = BackgroundFrame {
            level,
            w: cam.size.width,
            h: cam.size.height,
            resolution_mode: cam.size.resolution_mode(),
            // Read by level 74 only.
            now: tick,
            exiting: world.exit_requested,
            player_x,
            stars_seed: Some(
                self.stars_seed
                    .unwrap_or_else(|| stars_seed_at(time, tick, self.shake_start)),
            ),
            summit_seed: self.summit_seed,
            // The star tick is 0 before the first call (§12 l74 r3).
            stars_last: Some(0),
            palette: Some(&assets.palette),
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
