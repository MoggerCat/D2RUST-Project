// Spec: specs/render/draw-order.md (§10), specs/render/blend-modes.md (§5, §6), specs/render/shading.md (§3 r1, §4), specs/render/unit-composite.md (§8), specs/render/draw-order-2.md (§15); preview fills: docs/PLAN.md decision D1
//! The play preview (decision D1, `docs/handoff/first-playable-scope.md`
//! §4): labelled fills for the inputs the client model lacks, so the
//! first playable build draws the map instead of failing the frame. The
//! strict path ([`super::ModelFeed`] without a [`Preview`]) is unchanged
//! and stays the one the capture compare uses. Nothing drawn through a
//! fill is verified against 1.14d (CLAUDE.md rule 10).
//!
//! The fills, each `d2rs-own, unverified`:
//! - **Light:** lit by [`super::preview_light`] (the light map, PL2
//!   shades); with `D2RS_FULLBRIGHT=1` full bright, as follows.
//! - **Full-bright light:** Tiles draw with no light map (the source
//!   index unchanged, as a light byte of 0xFF, `shading.md` §3 r1); the
//!   feed states no frame light, so unit shade stays the rules'.
//! - **Unit facts:** zero unit flags and flag-ex, no states, objects
//!   `DrawUnder` 0, `unflatDead` off, and the sight test answers
//!   "visible" (`draw-order-2.md` §15 inputs are not in the model). With
//!   flag-ex 0 the fill lists no unit shadows (their draw is open question
//!   3 of `draw-order.md`).
//! - **Unit offsets:** no motion record and no table offset
//!   (`unit-composite.md` §8): `(0, 0)`.
//! - **Water floors:** material bit 0x2 is cleared in the near rooms, so
//!   the floor pass spawns no water effects (no weather state is fed).
//! - **Errors:** a tile whose art cannot be made resident is skipped (drawn
//!   as the empty [`skip_key`] frame) and logged once; a near-room build
//!   that fails leaves the frame without the map, logged once.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};

use crate::bridge::world::ClientWorld;
use crate::bridge::ClientUnit;
use crate::composite::ComponentFrame;
use crate::frames::{FramePart, FrameSet, FrameSetKey, IndexFrame};
use crate::rules::blend::{shadow_tile_ops, wall_draw, WallDraw};
use crate::rules::draw_order::source::TileArt;
use crate::rules::draw_order::{NearRooms, OrderedTile, TileArray, TileKind, UnitFacts};
use crate::rules::shading::ShadeTables;
use crate::scene::{BlendOp, DrawKey, ShadeChain};

use super::near_rooms::{Dt1Entry, MapState};
use super::tile_assets::{tile_key, TileAssets};
use super::{ViewAssets, ViewError};

/// The path of the empty frame a skipped item draws (not an archive path).
pub const SKIP_PATH: &str = "d2rs/preview/skip";

/// The key of the empty frame set: one 1 × 1 frame of index 0
/// (transparent).
pub fn skip_key() -> FrameSetKey {
    FrameSetKey::new(SKIP_PATH, FramePart::Tile(0)).expect("canonical")
}

/// Makes the empty frame set resident.
pub fn ensure_skip(assets: &mut ViewAssets) -> Result<(), ViewError> {
    let key = skip_key();
    if assets.frames.contains(&key) {
        return Ok(());
    }
    let frame = IndexFrame::new(1, 1, 0, 0, vec![0]).map_err(|e| ViewError::Unresolved {
        what: "preview skip frame",
        spec: "docs/PLAN.md D1",
        message: e.to_string(),
    })?;
    assets.frames.insert(
        key,
        FrameSet {
            frames: vec![frame],
        },
    )?;
    Ok(())
}

/// The art of a skipped tile: the empty frame, no blocks.
pub fn skipped_art() -> TileArt {
    TileArt {
        frame: ComponentFrame {
            set: skip_key(),
            index: 0,
        },
        blocks: Vec::new(),
        shade: ShadeChain::EMPTY,
        blend: BlendOp::Opaque,
    }
}

// d2rs-own, unverified (D1): the unit facts the model lacks are zero, the
// sight test answers visible (with the unit tables the feed runs the test
// of `draw-order-2.md` §15, `model_feed::preview_facts`). Flag-ex 0x80 is
// the last frame's sight test (`MapState` keeps it, `draw-order.md` §5 r3).
/// The facts of a room unit in the preview.
pub fn unit_facts(world: &ClientWorld, unit: &ClientUnit) -> UnitFacts {
    UnitFacts {
        unit_type: unit.key.unit_type,
        mode: unit.mode,
        local: world.local_player == Some(unit.key),
        sight_hidden: Some(false),
        ..UnitFacts::default()
    }
}

// d2rs-own, unverified (D1): no motion record, no table offset.
/// A unit's extra offset in the preview (`unit-composite.md` §8 with no
/// record and `TableOffset::None`).
pub fn unit_offset() -> (i32, i32) {
    (0, 0)
}

// d2rs-own, unverified (D1): full bright (no light map); the blend is the
// owner spec's for the record's kind and alpha byte. Shadow tiles draw
// blended (Blended Shadows on); without the act's tables they and
// translucent walls cannot be drawn as the spec says: shadow tiles are
// skipped, translucent walls drawn opaque.
/// The shade and blend of a tile of `kind` with alpha byte `alpha`, or
/// `None` when it is not drawn (`blend-modes.md` §6: alpha below 0x40;
/// a shadow tile without tables).
pub fn tile_ops(
    kind: TileKind,
    alpha: u8,
    tables: Option<&ShadeTables>,
) -> Option<(ShadeChain, BlendOp)> {
    match kind {
        TileKind::Floor { .. } => Some((ShadeChain::EMPTY, BlendOp::Opaque)),
        TileKind::ShadowTile => tables.map(|t| shadow_tile_ops(t, true)),
        TileKind::LowerWall | TileKind::Wall | TileKind::Roof { .. } => match wall_draw(alpha) {
            WallDraw::Lit => Some((ShadeChain::EMPTY, BlendOp::Opaque)),
            WallDraw::Hidden => None,
            WallDraw::Translucent(table) => Some((
                ShadeChain::EMPTY,
                tables.map_or(BlendOp::Opaque, |t| {
                    BlendOp::IndexTableSrcRow(table.base(t))
                }),
            )),
        },
    }
}

// d2rs-own, unverified (D1): no weather state is fed, so no water floor.
/// Clears the water bit (material 0x2) of every floor record.
pub fn dry_floors(near: &mut NearRooms) {
    for room in &mut near.rooms {
        for r in &mut room.floors {
            r.dt1.material &= !0x2;
        }
    }
}

/// The wall, floor and shadow record counts of each near room.
pub fn record_counts(near: Option<&NearRooms>) -> Vec<[usize; 3]> {
    near.map_or_else(Vec::new, |n| {
        n.rooms
            .iter()
            .map(|r| [r.walls.len(), r.floors.len(), r.shadows.len()])
            .collect()
    })
}

/// The DT1 entries of every record of the near rooms with `counts`.
pub fn entries(map: &MapState, counts: &[[usize; 3]]) -> Vec<Dt1Entry> {
    let mut out = Vec::new();
    for (ri, c) in counts.iter().enumerate() {
        for (array, n) in [
            (TileArray::Wall, c[0]),
            (TileArray::Floor, c[1]),
            (TileArray::Shadow, c[2]),
        ] {
            out.extend((0..n).filter_map(|i| map.entry(ri, array, i).cloned()));
        }
    }
    // The act's edge floor record (`draw-order-2.md` §14).
    out.extend(map.entry(0, TileArray::Edge, 0).cloned());
    out
}

/// The preview's state in the model feed: the tile art and the log of
/// what was skipped.
#[derive(Debug, Clone, Default)]
pub struct Preview {
    pub tiles: TileAssets,
    /// The palette act of the last prepared frame (its shade tables).
    act: u8,
    /// The preview's light (`preview_light`); full bright with
    /// `D2RS_FULLBRIGHT=1`.
    pub light: super::preview_light::PreviewLight,
    logged: Arc<Mutex<BTreeSet<String>>>,
    /// The frame's per-block shades by draw key (`preview_blocks`), filled
    /// by [`Self::tile_art`], read by `ViewSource::tile_blocks`.
    block_shades: Arc<Mutex<BTreeMap<DrawKey, Vec<crate::rules::BlockShade>>>>,
}

impl Preview {
    pub fn new(tiles: TileAssets) -> Self {
        Preview {
            tiles,
            light: super::preview_light::PreviewLight::new(),
            ..Preview::default()
        }
    }

    /// Stores the per-block shades of the tile with draw key `key`.
    pub(crate) fn put_block_shades(&self, key: DrawKey, shades: Vec<crate::rules::BlockShade>) {
        if let Ok(mut b) = self.block_shades.lock() {
            b.insert(key, shades);
        }
    }

    /// The per-block shades of the tile with draw key `key` this frame.
    pub fn block_shades_of(&self, key: DrawKey) -> Vec<crate::rules::BlockShade> {
        self.block_shades
            .lock()
            .ok()
            .and_then(|b| b.get(&key).cloned())
            .unwrap_or_default()
    }

    /// Logs `message` the first time it is seen; returns whether it was new.
    pub fn log_once(&self, message: String) -> bool {
        let new = self
            .logged
            .lock()
            .map(|mut l| l.insert(message.clone()))
            .unwrap_or(false);
        if new {
            bevy::log::warn!("preview (d2rs-own, unverified): skipped: {message}");
        }
        new
    }

    /// Everything logged so far (tests, the run summary).
    pub fn logged(&self) -> Vec<String> {
        self.logged
            .lock()
            .map(|l| l.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Before the frame's build: the act's shade tables, the empty frame
    /// and the near rooms' DT1 `entries` resident; failures logged.
    pub fn prepare(
        &mut self,
        world: &ClientWorld,
        entries: &[Dt1Entry],
        assets: &mut ViewAssets,
    ) -> Result<(), ViewError> {
        self.act = world.palette_act.unwrap_or(0);
        if let Ok(mut b) = self.block_shades.lock() {
            b.clear();
        }
        if let Err(m) = self.tiles.ensure_shades(self.act, &mut assets.maps) {
            self.log_once(m);
        }
        // The act's tables when the tiles hold them; else the UI path's
        // (`app::ui::push_text_colors`) stay: the UI rectangles of draw
        // mode 2 read them (`blend-modes.md` §1).
        if let Some(t) = self.tiles.shades(self.act).copied() {
            assets.shades = Some(t);
        }
        ensure_skip(assets)?;
        for m in self.tiles.ensure(entries, assets) {
            self.log_once(m);
        }
        Ok(())
    }

    /// The art of an ordered tile: its DT1 frame and blocks with the
    /// preview's [`tile_ops`]; a tile that is not resident is skipped
    /// ([`skipped_art`]), and so is one that is not drawn.
    pub fn tile_art(&self, map: Option<&MapState>, tile: &OrderedTile) -> TileArt {
        let Some((path, index)) = map.and_then(|m| m.entry(tile.room, tile.array, tile.record))
        else {
            self.log_once(format!(
                "no DT1 entry for room {} {:?} record {}",
                tile.room, tile.array, tile.record
            ));
            return skipped_art();
        };
        let key = match tile_key(path, *index) {
            Ok(k) => k,
            Err(m) => {
                self.log_once(m);
                return skipped_art();
            }
        };
        let Some(blocks) = self.tiles.blocks(&key) else {
            // `prepare` logged why (or the entry appeared after it ran).
            if self.tiles.failure(&key).is_none() {
                self.log_once(format!("{} tile {index}: not resident", key.path()));
            }
            return skipped_art();
        };
        let Some((shade, blend)) = tile_ops(tile.kind, tile.alpha, self.tiles.shades(self.act))
        else {
            return skipped_art();
        };
        // Lit (preview_light, d2rs-own, unverified): a tile's light is one
        // flat value; shadow tiles keep their blend-table chain.
        let shade = match tile.kind {
            TileKind::ShadowTile => shade,
            _ => self
                .light
                .tile_chain(tile.kind, &tile.dt1, tile.cell)
                .unwrap_or(shade),
        };
        if !matches!(tile.kind, TileKind::ShadowTile) {
            let grids = self.tiles.grids(&key).unwrap_or_default();
            let shades = self.light.block_shades(
                tile.kind,
                &tile.dt1,
                tile.cell,
                (tile.alpha, tile.fade_state),
                blend,
                blocks,
                grids,
            );
            if !shades.is_empty() {
                if let Ok(k) = DrawKey::new(tile.key.pass, tile.key.major, tile.key.minor, 0) {
                    self.put_block_shades(k, shades);
                }
            }
        }
        TileArt {
            frame: ComponentFrame { set: key, index: 0 },
            blocks: blocks.to_vec(),
            shade,
            blend,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bridge::world::{MONSTER, PLAYER};
    use crate::bridge::UnitKey;
    use crate::rules::shading::ShadeTables;
    use crate::scene::MapTable;

    fn tables() -> ShadeTables {
        let pl2 =
            d2_formats::palette::Pl2::parse(&super::super::tile_assets::tests::pl2()).unwrap();
        ShadeTables::push(&mut MapTable::new(), &pl2)
    }

    // Covers: specs/render/blend-modes.md §6
    #[test]
    fn tile_ops_are_full_bright_with_the_kinds_blend() {
        let t = tables();
        let floor = TileKind::Floor { layer: 1 };
        assert_eq!(
            tile_ops(floor, 0x10, None),
            Some((ShadeChain::EMPTY, BlendOp::Opaque))
        );
        assert_eq!(
            tile_ops(TileKind::Wall, 0xFF, Some(&t)),
            Some((ShadeChain::EMPTY, BlendOp::Opaque))
        );
        assert_eq!(tile_ops(TileKind::Roof { pass: 1 }, 0x3F, Some(&t)), None);
        assert_eq!(
            tile_ops(TileKind::LowerWall, 0x80, Some(&t)),
            Some((ShadeChain::EMPTY, BlendOp::IndexTableSrcRow(t.alpha[1])))
        );
        assert_eq!(
            tile_ops(TileKind::Wall, 0xC0, None),
            Some((ShadeChain::EMPTY, BlendOp::Opaque))
        );
        assert_eq!(tile_ops(TileKind::ShadowTile, 0xFF, None), None);
        assert_eq!(
            tile_ops(TileKind::ShadowTile, 0xFF, Some(&t)),
            Some((ShadeChain::EMPTY, BlendOp::IndexTable(t.alpha[0])))
        );
    }

    // Covers: specs/render/draw-order.md §5 r3
    #[test]
    fn unit_facts_are_zero_and_visible() {
        let mut w = ClientWorld::default();
        let key = UnitKey::new(PLAYER, 7);
        let mut u = ClientUnit::new(key);
        u.mode = 2;
        w.local_player = Some(key);
        let f = unit_facts(&w, &u);
        assert_eq!(
            f,
            UnitFacts {
                unit_type: PLAYER,
                mode: 2,
                local: true,
                sight_hidden: Some(false),
                // Flag-ex 0x80 is the last frame's sight test, kept by
                // `MapState` (§5 r3), not a fact of the unit.
                ..UnitFacts::default()
            }
        );
        let m = ClientUnit::new(UnitKey::new(MONSTER, 7));
        let f = unit_facts(&w, &m);
        assert!(!f.local);
        // A sight-tested unit draws (no shadow entry: flag-ex 0).
        let mut f = f;
        f.mode = 1;
        assert!(crate::rules::draw_order::unit_draws(&mut f).unwrap());
        assert_eq!(unit_offset(), (0, 0));
    }

    // Covers: specs/client/assets.md §a4-residency
    #[test]
    fn skip_frame_is_resident_once_and_logs_are_deduplicated() {
        let mut a = ViewAssets::new(crate::app::play::unspecified_palette());
        ensure_skip(&mut a).unwrap();
        ensure_skip(&mut a).unwrap();
        let f = a.frame(&skip_key(), 0).unwrap();
        assert_eq!((f.width, f.height), (1, 1));
        let p = Preview::default();
        assert!(p.log_once("x".into()));
        assert!(!p.log_once("x".into()));
        assert_eq!(p.logged(), ["x"]);
    }
}
