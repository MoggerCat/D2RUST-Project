// Spec: specs/render/unit-composite.md (§9), specs/render/camera.md (§2, §3, §4), specs/render/sprite-placement.md (§2, §8), specs/render/draw-order.md (§3, §10), specs/items/inventory-moves.md (§7.1, §7.2), specs/ui/controls.md (§6 r4)
//! Ground items in the play preview's world view: each item of the model
//! in mode 3 / 5 drawn at its sub-tile with its flippy DC6, and the world
//! clicks that pick one up (C→S 0x16) or drop the cursor item (C→S 0x17).
//!
//! What follows the specs:
//! - the art file: the item's `flippyfile`, path
//!   `data\global\items\<name>.dc6` (`unit-composite.md` §9);
//! - the position: a static unit at its sub-tile (`camera.md` §2), drawn at
//!   the camera's unit draw position with no extra offset (§4), the DC6
//!   cel placed bottom-anchored (`sprite-placement.md` §2, §8);
//! - the pass: a ground item is a flat unit in the shadow list, drawn in
//!   pass 5 (`draw-order.md` §3, §10);
//! - draw mode 5 (not highlighted): the opaque copy (`unit-composite.md`
//!   §9, `render/blend-modes.md`);
//! - a left press in the world while the local player holds a cursor item
//!   sends 0x17 with its GUID (`controls.md` §6 r4 kind 0;
//!   `inventory-moves.md` §7.2).
//!
//! d2rs-own, unverified (decision D1, the play preview's fills):
//! - the cel is the flippy's last frame (the resting pose): the flip
//!   animation's frame and timing need inputs the model lacks;
//! - the unique / set flippy override and the gold amount class
//!   (`unit-composite.md` §9) are not applied: the model's item view has
//!   no quality, unique / set row or stat 14;
//! - no colormap and no light (`0x0062C100`, `render/lighting.md`): the
//!   cel is unshaded;
//! - the draw key: pass 5 with major `DrawKey::MAJOR_MAX` (after the
//!   shadow tiles of every cell) and minor by (sx + sy, GUID), not the
//!   draw-cell grid index of `draw-order.md` §2;
//! - no shake in the camera (no effect spec starts one, `model_feed.rs`
//!   `PENDING`);
//! - the click on an item: the original hovers the unit
//!   (`0x00467A10`, no hover model yet) and walks to it / interacts
//!   (`controls.md` §6 r9.2, item: 0x13 or a walk-to-unit code); the
//!   preview sends 0x16 PickItem with cursor flag 0 (the server walks the
//!   player to the item, `inventory-moves.md` §7.1 r2.2) when the press
//!   lands inside the art rectangle of a drawn item (topmost in draw
//!   order);
//! - a file that is missing or fails to parse is logged once and its
//!   items are not drawn.
//!
//! No name labels: no spec states the ground-item label (Alt,
//! `CfgShowItems` sets UI state 0xD, `controls.md` §3; its drawer is not
//! specified).
//!
//! The client decides no outcome here (CLAUDE.md rule 7): the draws read
//! the model, the clicks are requests the server checks.

use std::collections::BTreeMap;
use std::sync::Arc;

use crate::assets::path::{CanonicalPath, FileSource};
use crate::bridge::items::{self, ItemArtRows};
use crate::bridge::link::ServerLink;
use crate::bridge::world::ClientWorld;
use crate::bridge::{Bridge, BridgeError};
use crate::frames::{FramePart, FrameSet, FrameSetKey};
use crate::rules::camera::{static_to_client, Camera, FrameSize};
use crate::rules::placement::place;
use crate::scene::order::pass;
use crate::scene::{BlendOp, DrawItem, DrawKey, ItemTag, Rect, ShadeChain};
use crate::ui::{PointerButton, UiEvent};

use super::feed::ViewFeed;
use super::{ViewAssets, WorldFrame};

/// The open mode whose frames draw no world (`composition.md` §3 step 3).
const NO_WORLD_MODE: u8 = 3;

/// The archive name of a flippy file (`unit-composite.md` §9).
pub fn flippy_archive_name(name: &str) -> String {
    format!("data\\global\\items\\{name}.dc6")
}

/// One drawn ground item: its draw and the screen rectangle its art
/// covers (inside the frame).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GroundDraw {
    pub guid: u32,
    pub item: DrawItem,
    pub hit: Rect,
}

/// The ground-item state of the world view: the item art rows (handed in
/// by the app), the file source the flippy files are read from, the files
/// loaded so far and the art rectangles of the last drawn frame (for the
/// clicks). The default (no rows, no source) draws nothing.
#[derive(Default)]
pub struct GroundItems {
    rows: ItemArtRows,
    source: Option<Arc<dyn FileSource>>,
    /// Flippy name → its frame set and frame count; `None`: tried and
    /// missing or refused.
    files: BTreeMap<String, Option<(FrameSetKey, usize)>>,
    /// The last drawn frame's items, in draw order.
    last: Vec<GroundDraw>,
}

impl std::fmt::Debug for GroundItems {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GroundItems")
            .field("rows", &self.rows.0.len())
            .field("source", &self.source.is_some())
            .field("files", &self.files)
            .field("last", &self.last)
            .finish()
    }
}

impl GroundItems {
    /// The state with the item art rows and the archives to read the
    /// flippy files from.
    pub fn new(source: Arc<dyn FileSource>, rows: ItemArtRows) -> Self {
        GroundItems {
            rows,
            source: Some(source),
            files: BTreeMap::new(),
            last: Vec::new(),
        }
    }

    /// Replaces the item art rows (files already loaded stay).
    pub fn set_rows(&mut self, rows: ItemArtRows) {
        self.rows = rows;
    }

    /// Replaces the file source; files not yet tried are read from it.
    pub fn set_source(&mut self, source: Arc<dyn FileSource>) {
        self.source = Some(source);
    }

    /// The last drawn frame's ground items, in draw order.
    pub fn last(&self) -> &[GroundDraw] {
        &self.last
    }

    /// The flippy name of a ground item, `None` without a row or a name.
    fn flippy(&self, code: Option<[u8; 4]>) -> Option<&str> {
        let row = self.rows.get(code?)?;
        let name = row.flippy_file.trim();
        (!name.is_empty()).then_some(name)
    }

    /// Makes the flippy file of every ground item resident, once each.
    /// Returns one log line per new failure (the file's items are not
    /// drawn, d2rs-own).
    pub fn ensure(&mut self, world: &ClientWorld, assets: &mut ViewAssets) -> Vec<String> {
        let mut log = Vec::new();
        let Some(source) = self.source.clone() else {
            return log;
        };
        for item in items::ground_items(world) {
            let Some(name) = self.flippy(item.code).map(str::to_owned) else {
                continue;
            };
            if self.files.contains_key(&name) {
                continue;
            }
            let loaded = load(source.as_ref(), &name, assets);
            if let Err(e) = &loaded {
                log.push(format!(
                    "ground item art: {}: {e}",
                    flippy_archive_name(&name)
                ));
            }
            self.files.insert(name, loaded.ok());
        }
        log
    }

    /// The draws of the ground items whose art is resident, under
    /// `camera`, in unit-key order (module doc).
    pub fn draws(
        &self,
        world: &ClientWorld,
        camera: &Camera,
        assets: &ViewAssets,
    ) -> Vec<GroundDraw> {
        let mut found = Vec::new();
        for item in items::ground_items(world) {
            let Some(name) = self.flippy(item.code) else {
                continue;
            };
            let Some(Some((set, frames))) = self.files.get(name) else {
                continue;
            };
            let Some(index) = frames.checked_sub(1) else {
                continue;
            };
            let (Ok(id), Ok(image)) = (assets.id(set, index), assets.frame(set, index)) else {
                continue;
            };
            let (sx, sy) = (i32::from(item.x), i32::from(item.y));
            let (x, y) = camera.unit_draw(static_to_client(sx, sy), (0, 0));
            let placed = place(image, x, y, Rect::FRAME);
            let Some(clip) = placed.clip else { continue };
            let art = Rect::new(placed.x, placed.y, image.width, image.height);
            let Some(hit) = art.intersect(&clip) else {
                continue;
            };
            let mut d = DrawItem::new(id, placed.x, placed.y);
            d.clip = clip;
            d.shade = ShadeChain::EMPTY;
            d.blend = BlendOp::Opaque;
            d.tag = ItemTag::Unit(item.key.guid);
            found.push((sx + sy, item.key.guid, d, hit));
        }
        found.sort_by_key(|&(depth, guid, ..)| (depth, guid));
        found
            .into_iter()
            .enumerate()
            .filter_map(|(minor, (_, guid, mut item, hit))| {
                let minor = u32::try_from(minor).ok()?;
                item.key = DrawKey::new(pass::SHADOWS, DrawKey::MAJOR_MAX, minor, 0).ok()?;
                Some(GroundDraw { guid, item, hit })
            })
            .collect()
    }

    /// Loads the art ([`Self::ensure`]) and adds the ground items to a
    /// built `frame` (re-sorted by key), under the camera of `feed`
    /// (`camera.md` §3, no shake). No local player, or open mode 3 (no
    /// world): nothing. Never fails the frame; returns log lines.
    pub fn add_to_frame<F: ViewFeed + ?Sized>(
        &mut self,
        world: &ClientWorld,
        feed: &F,
        assets: &mut ViewAssets,
        frame: &mut WorldFrame,
    ) -> Vec<String> {
        self.last.clear();
        if self.rows.0.is_empty() {
            return Vec::new();
        }
        let mut log = self.ensure(world, assets);
        let camera = match (feed.player(world), feed.open_mode(world)) {
            (Ok(Some(p)), Ok(mode)) if mode.get() != NO_WORLD_MODE => {
                Camera::new(FrameSize::D2RS, mode, p.client(), (0, 0))
            }
            (Err(e), _) | (_, Err(e)) => {
                log.push(format!("ground items: no camera: {e}"));
                return log;
            }
            _ => return log,
        };
        self.last = self.draws(world, &camera, assets);
        if !self.last.is_empty() {
            frame.items.extend(self.last.iter().map(|d| d.item));
            crate::scene::order(&mut frame.items);
        }
        log
    }

    /// The ground item whose art (last drawn frame) covers screen point
    /// `(x, y)`: the topmost, the last in draw order.
    pub fn hit(&self, x: i32, y: i32) -> Option<u32> {
        self.last
            .iter()
            .rev()
            .find(|d| d.hit.contains(i64::from(x), i64::from(y)))
            .map(|d| d.guid)
    }

    /// The world clicks of the item rules, before the world-click
    /// dispatcher: a left press while the local player holds a cursor item
    /// → C→S 0x17 (`controls.md` §6 r4); a left press on a drawn ground
    /// item → C→S 0x16, cursor flag 0 (d2rs-own, module doc). The events
    /// taken are left out of the returned list, the rest go on to the
    /// dispatcher.
    pub fn take_clicks<L: ServerLink>(
        &self,
        bridge: &mut Bridge<L>,
        unhandled: &[UiEvent],
    ) -> Result<Vec<UiEvent>, BridgeError> {
        let mut rest = Vec::with_capacity(unhandled.len());
        for e in unhandled {
            if let UiEvent::Press {
                button: PointerButton::Left,
                at,
            } = *e
            {
                if let Some(held) = items::cursor_item(bridge.world()) {
                    bridge.send(&items::drop(held.key.guid))?;
                    continue;
                }
                if let Some(guid) = self.hit(at.x, at.y) {
                    bridge.send(&items::pick(guid, false))?;
                    continue;
                }
            }
            rest.push(*e);
        }
        Ok(rest)
    }
}

/// Reads flippy `name` (direction 0 of its DC6) into the frame store.
fn load(
    source: &dyn FileSource,
    name: &str,
    assets: &mut ViewAssets,
) -> Result<(FrameSetKey, usize), String> {
    let archive = flippy_archive_name(name);
    let path = CanonicalPath::new(&archive).map_err(|e| e.to_string())?;
    let key = FrameSetKey::new(path.as_str(), FramePart::Dir(0)).map_err(|e| e.to_string())?;
    let dc6 = crate::assets::path::read_dc6(source, &archive)
        .ok_or_else(|| "in no archive".to_string())??;
    let frames = dc6.header.frames_per_direction as usize;
    if !assets.frames.contains(&key) {
        let set = FrameSet::from_dc6(&dc6, 0).map_err(|e| e.to_string())?;
        assets
            .frames
            .insert(key.clone(), set)
            .map_err(|e| e.to_string())?;
    }
    Ok((key, frames))
}

#[cfg(test)]
#[path = "ground_items_tests.rs"]
mod tests;
