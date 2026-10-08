// Spec: specs/client/model.md (§3 rule 3, §1 rule 2, §11 rules 3, 5), specs/render/camera.md (§2, §3)
//! [`ModelFeed`]: the [`ViewFeed`] hooks the client world model answers
//! (RW2): the local player's position (camera §3) and unit positions
//! (camera §2), given the `Levels.txt` rows, BlankScreen of the local
//! player's level (`model.md` §11 rule 5), and, once the world view hands
//! it over, the original UI's open mode (`ui/panels.md` §4.2). Every
//! other hook (shake, player seed, unit offsets, map tiles, light,
//! weather) goes to the wrapped feed, [`NoFeed`] by default; why the model
//! cannot answer them yet is [`PENDING`].
//!
//! With a [`Preview`] (the play preview, decision D1) the map is on and
//! the tile art, unit facts and unit offsets are the labelled fills of
//! [`preview`] (`d2rs-own, unverified`); without one every answer above
//! is the strict one.
//!
//! Positions are the model's cells (`ClientUnit::position`). A moving
//! unit (players, monsters, missiles) is on a dynamic path: its 16.16
//! position is the cell centre after every placement the message specs
//! state (`model.md` §3 rule 3); a unit created at (0, 0) has its path at
//! (0, 0). The walk prediction between messages is not a message rule
//! (`model.md` open question 2), so the position changes only when a
//! message places the unit. Static units (objects, items, tiles) are at
//! their cell; an item without a cell (not on the ground, until the item
//! stream is specified) has no position.

use d2_sim::rng::Seed;

use crate::bridge::drlg::DrlgRoomId;
use crate::bridge::world::{ClientWorld, LevelRow, UnitKey, ITEM, OBJECT, TILE};
use crate::bridge::ClientUnit;
use crate::rules::draw_order::{FadeClock, NearRooms, UnitFacts};
use crate::rules::{MapTile, OpenMode, UnitPosition, ViewSource};

use super::feed::{NoFeed, RunningShake, ViewFeed};
use super::near_rooms::MapState;
use super::preview::{self, Preview};
use super::{UnitPose, ViewAssets, ViewError};

/// The 16.16 position of a dynamic path at the centre of cell `c`
/// (`sim/path-placement.md` §1 rule 2).
fn centre(c: u16) -> u32 {
    (u32::from(c) << 16) | 0x8000
}

/// The position of `unit` as the client keeps it (camera §2).
pub fn unit_position(unit: &ClientUnit) -> Result<UnitPosition, String> {
    let static_kind = matches!(unit.key.unit_type, OBJECT | ITEM | TILE);
    match (static_kind, unit.position) {
        (true, Some((x, y))) => Ok(UnitPosition::Static {
            sx: i32::from(x),
            sy: i32::from(y),
        }),
        (true, None) => Err(format!(
            "unit ({}, {}) has no cell (an item off the ground has no world position, msg-stats-items.md §2 r4)",
            unit.key.unit_type, unit.key.guid
        )),
        (false, p) => {
            let (x, y) = p.unwrap_or((0, 0));
            Ok(UnitPosition::Moving {
                x16: centre(x),
                y16: centre(y),
            })
        }
    }
}

/// The [`ViewFeed`] / model hooks the client model cannot fill yet, each
/// with the input it lacks (M02: named, never guessed). [`ModelFeed`]
/// leaves them to `inner` ([`NoFeed`]: none).
pub const PENDING: &[(&str, &str)] = &[
    (
        "ViewFeed::near_rooms (room unit facts)",
        "the near rooms are built from the client DRLG (`ModelFeed::with_map`, `near_rooms.rs`: \
         rooms, tile records with roof height and height, coordinate records, persisted fades, \
         the room unit lists in the client's order with the fill's Y sort written back), but a \
         listed unit's flags (+0xC4), flag-ex (+0xC8), monstats2 `unflatDead`, objects \
         `DrawUnder`, states 7 / 143 / 146 and the sight test (`draw-order-2.md` §15) are not in \
         the model (`ViewFeed::unit_facts` refuses)",
    ),
    (
        "ViewFeed::tile_art, ViewSource::tile_blocks",
        "each record's DT1 entry (path, index) is known (`rooms.md` §9.3, `MapState::entry`), \
         but the art's shade and blend and the per-block shading need the frame's light \
         (below); so the app runs the feed without the map (`ModelFeed::map` off)",
    ),
    (
        "ViewFeed::light",
        "the light map needs the act environment (S→C 0x53 has no client handler), light \
         records and the per-unit look inputs (fade, ghostly, hover, items, remaps); none is in \
         the model (the record list `ClientWorld::lights` exists and the act room callback runs \
         over it, but no unit code creates records)",
    ),
    (
        "ViewFeed::weather_frame",
        "the player's level is known now (`model.md` §11 r5), but no water floor is drawn \
         without tile art (above) and passes 4 / 9 have no art path yet",
    ),
    (
        "ViewFeed::player_seed, ViewFeed::shake",
        "the local player's client seed is read-only in the model (`camera.md` open question 6); \
         no effect spec starts a shake",
    ),
];

/// The client world model's answers, over `inner` for the rest.
#[derive(Debug, Clone, Default)]
pub struct ModelFeed<F = NoFeed> {
    pub inner: F,
    /// The `Levels.txt` rows by level id. `None`: BlankScreen goes to
    /// `inner` (the app supplies no rows yet).
    pub levels: Option<Vec<LevelRow>>,
    /// The original UI's open mode (`ui/panels.md` §4.2), once the world
    /// view has handed one over; `None`: `open_mode` goes to `inner`.
    pub ui_open_mode: Option<OpenMode>,
    /// The map from the client DRLG (`draw-order.md` §9). `None` (the
    /// default): `near_rooms` goes to `inner`. Opt-in ([`Self::with_map`])
    /// while tile art is pending: with near rooms every drawn tile needs
    /// [`ViewFeed::tile_art`] (`PENDING`).
    pub map: Option<MapState>,
    /// The play preview's fills (decision D1, [`preview`]): tile art,
    /// unit facts and offsets the model lacks, each `d2rs-own,
    /// unverified`. `None` (the default): the strict path.
    pub preview: Option<Preview>,
    /// The local player's predicted 16.16 position (decision D2,
    /// `bridge::predict`); read only with a [`Preview`].
    pub local_at: Option<(UnitKey, (u32, u32))>,
}

impl<F> ModelFeed<F> {
    pub fn new(inner: F) -> Self {
        Self {
            inner,
            levels: None,
            ui_open_mode: None,
            map: None,
            preview: None,
            local_at: None,
        }
    }

    /// The feed with the map and the play preview's fills
    /// ([`preview`]).
    pub fn with_preview(mut self, preview: Preview) -> Self {
        self.map.get_or_insert_with(MapState::default);
        self.preview = Some(preview);
        self
    }

    /// The feed with the client DRLG's near rooms (`draw-order.md` §9).
    pub fn with_map(mut self) -> Self {
        self.map = Some(MapState::default());
        self
    }
}

impl<F> ModelFeed<F> {
    /// The unit's position: with a preview, the local player is at its
    /// predicted position (d2rs-own, unverified: decision D2); otherwise
    /// [`unit_position`].
    fn position_of(&self, unit: &ClientUnit) -> Result<UnitPosition, String> {
        match (&self.preview, self.local_at) {
            (Some(_), Some((key, (x16, y16)))) if key == unit.key => {
                Ok(UnitPosition::Moving { x16, y16 })
            }
            _ => unit_position(unit),
        }
    }
}

impl<F: ViewSource> ViewSource for ModelFeed<F> {
    fn unit_position(&self, unit: &ClientUnit) -> Result<UnitPosition, String> {
        self.position_of(unit)
    }

    /// Preview: `(0, 0)` ([`preview::unit_offset`]); else the inner
    /// feed's.
    fn unit_offset(&self, unit: &ClientUnit, pose: &UnitPose) -> Result<(i32, i32), String> {
        if self.preview.is_some() {
            return Ok(preview::unit_offset());
        }
        self.inner.unit_offset(unit, pose)
    }

    fn map_tiles(
        &self,
        world: &ClientWorld,
        assets: &ViewAssets,
    ) -> Result<Vec<MapTile>, ViewError> {
        self.inner.map_tiles(world, assets)
    }

    fn tile_blocks(&self, tile: &MapTile) -> Result<Vec<crate::rules::BlockShade>, ViewError> {
        self.inner.tile_blocks(tile)
    }
}

impl<F: ViewFeed> ViewFeed for ModelFeed<F> {
    /// `model.md` §3 rule 3: the local player's position; no local player
    /// → `None`.
    fn player(&self, world: &ClientWorld) -> Result<Option<UnitPosition>, ViewError> {
        let Some(unit) = world.local() else {
            return Ok(None);
        };
        self.position_of(unit)
            .map(Some)
            .map_err(|message| ViewError::Unresolved {
                what: "local player position",
                spec: "client/model.md",
                message,
            })
    }

    /// `ui/panels.md` §4.2: the UI flags' open mode (camera §1); without
    /// the original UI, the inner feed's answer.
    fn open_mode(&self, world: &ClientWorld) -> Result<OpenMode, ViewError> {
        match self.ui_open_mode {
            Some(m) => Ok(m),
            None => self.inner.open_mode(world),
        }
    }

    fn set_ui_open_mode(&mut self, mode: OpenMode) {
        self.ui_open_mode = Some(mode);
    }

    fn set_local_prediction(&mut self, at: Option<(UnitKey, (u32, u32))>) {
        self.local_at = at;
    }

    /// The preview places cels cut by the frame edge (D1).
    fn edge_clip(&self) -> bool {
        self.preview.is_some()
    }

    fn shake(&self, world: &ClientWorld) -> Result<Option<RunningShake>, ViewError> {
        self.inner.shake(world)
    }

    fn player_seed(&mut self, world: &ClientWorld) -> Result<&mut Seed, ViewError> {
        self.inner.player_seed(world)
    }

    /// `model.md` §11 rules 3, 5: BlankScreen of the level of the local
    /// player's room; no room → no level, BlankScreen 0. The level never
    /// comes from 0x03 u16@6 or a 0x07 level byte.
    fn blank_screen(&self, world: &ClientWorld) -> Result<bool, ViewError> {
        let Some(levels) = &self.levels else {
            return self.inner.blank_screen(world);
        };
        let Some(level) = world.player_level() else {
            return Ok(false);
        };
        levels
            .get(usize::from(level))
            .map(|r| r.blank_screen)
            .ok_or(ViewError::Unresolved {
                what: "player level BlankScreen",
                spec: "client/model.md",
                message: format!("level {level} past the Levels rows"),
            })
    }

    /// Preview: the frame's light and look (`preview_light`), unless
    /// full bright; else the inner feed's.
    fn light(&self, world: &ClientWorld) -> Result<Option<super::feed::FeedLight<'_>>, ViewError> {
        if let Some(p) = &self.preview {
            return Ok(p.light.frame().map(|light| super::feed::FeedLight {
                light,
                look: p.light.look(),
            }));
        }
        self.inner.light(world)
    }

    /// `draw-order.md` §9 from the client DRLG when the map is on
    /// ([`MapState`]); the unit facts the model holds (type, mode, local
    /// player) over the inner feed's for the rest.
    /// Preview: the facts are [`preview::unit_facts`], the floors are
    /// dried ([`preview::dry_floors`]) and a build that fails is logged
    /// once and leaves the frame without the map.
    fn near_rooms(&mut self, world: &ClientWorld) -> Result<Option<&mut NearRooms>, ViewError> {
        let Self {
            inner,
            levels,
            map,
            preview,
            ..
        } = self;
        let Some(map) = map.as_mut() else {
            return inner.near_rooms(world);
        };
        let Some(preview) = preview.as_ref() else {
            let inner = &*inner;
            return map.near_rooms(world, levels.as_deref(), |u| {
                Ok(UnitFacts {
                    unit_type: u.key.unit_type,
                    mode: u.mode,
                    local: world.local_player == Some(u.key),
                    ..inner.unit_facts(world, u)?
                })
            });
        };
        match map.near_rooms(world, levels.as_deref(), |u| {
            Ok(preview::unit_facts(world, u))
        }) {
            Ok(Some(near)) => {
                preview::dry_floors(near);
                Ok(Some(near))
            }
            Ok(None) => Ok(None),
            Err(e) => {
                preview.log_once(format!("the map this frame: {e}"));
                Ok(None)
            }
        }
    }

    fn unit_facts(&self, world: &ClientWorld, unit: &ClientUnit) -> Result<UnitFacts, ViewError> {
        if self.preview.is_some() {
            return Ok(preview::unit_facts(world, unit));
        }
        self.inner.unit_facts(world, unit)
    }

    /// Preview: the act's shade tables, the skip frame and the near rooms'
    /// DT1 tiles resident ([`Preview::prepare`]); else the inner feed's.
    fn prepare(&mut self, world: &ClientWorld, assets: &mut ViewAssets) -> Result<(), ViewError> {
        if self.preview.is_none() {
            return self.inner.prepare(world, assets);
        }
        let counts = preview::record_counts(self.near_rooms(world)?.as_deref());
        let entries = match &self.map {
            Some(m) => preview::entries(m, &counts),
            None => Vec::new(),
        };
        let local_at = self.local_at;
        let preview = self.preview.as_mut().expect("checked above");
        let r = preview.prepare(world, &entries, assets);
        let tables = preview
            .tiles
            .shades(world.palette_act.unwrap_or(0))
            .copied();
        preview.light.refresh(world, local_at, tables.as_ref());
        r
    }

    fn take_unit_orders(&mut self) -> Vec<(DrlgRoomId, Vec<UnitKey>)> {
        match self.map.as_mut() {
            Some(m) => m.take_unit_orders(),
            None => self.inner.take_unit_orders(),
        }
    }

    fn weather_frame(
        &mut self,
        world: &ClientWorld,
    ) -> Result<Option<crate::rules::draw_order::source::WeatherFrame<'_>>, ViewError> {
        self.inner.weather_frame(world)
    }

    fn fade_clock(&self, world: &ClientWorld) -> Result<FadeClock, ViewError> {
        self.inner.fade_clock(world)
    }

    fn tile_art(
        &self,
        tile: &crate::rules::draw_order::OrderedTile,
        assets: &ViewAssets,
    ) -> Result<crate::rules::draw_order::source::TileArt, ViewError> {
        match &self.preview {
            Some(p) => Ok(p.tile_art(self.map.as_ref(), tile)),
            None => self.inner.tile_art(tile, assets),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bridge::world::{MONSTER, PLAYER};
    use crate::bridge::UnitKey;

    fn unit(unit_type: u8, position: Option<(u16, u16)>) -> ClientUnit {
        let mut u = ClientUnit::new(UnitKey::new(unit_type, 1));
        u.position = position;
        u
    }

    // Covers: specs/client/model.md §3 r3
    #[test]
    fn local_player_position_is_the_cell_centre() {
        let feed = ModelFeed::<NoFeed>::default();
        let mut w = ClientWorld::default();
        assert_eq!(feed.player(&w).unwrap(), None);
        let key = UnitKey::new(PLAYER, 1);
        w.units.insert(key, unit(PLAYER, Some((0x1241, 0x11C4))));
        // A unit, but not the local player: no camera.
        assert_eq!(feed.player(&w).unwrap(), None);
        w.local_player = Some(key);
        assert_eq!(
            feed.player(&w).unwrap(),
            Some(UnitPosition::Moving {
                x16: 0x1241_8000,
                y16: 0x11C4_8000
            })
        );
        // Created at (0, 0): the path is at (0, 0).
        w.units.get_mut(&key).unwrap().position = None;
        assert_eq!(
            feed.player(&w).unwrap(),
            Some(UnitPosition::Moving {
                x16: 0x8000,
                y16: 0x8000
            })
        );
        // Open mode without the UI: 0 (ui/panels-2.md §22 r5).
        assert_eq!(feed.open_mode(&w).unwrap(), OpenMode::NONE);
    }

    // Covers: specs/ui/panels.md §4 r2
    #[test]
    fn open_mode_is_the_uis_once_handed_over() {
        let mut feed = ModelFeed::<NoFeed>::default();
        let w = ClientWorld::default();
        assert_eq!(
            feed.open_mode(&w).unwrap(),
            OpenMode::NONE,
            "no UI: the inner feed's answer"
        );
        feed.set_ui_open_mode(OpenMode::new(3).unwrap());
        assert_eq!(feed.open_mode(&w).unwrap().get(), 3);
        // A feed that ignores the UI keeps its own answer.
        let mut inner = NoFeed;
        inner.set_ui_open_mode(OpenMode::new(1).unwrap());
        assert_eq!(inner.open_mode(&w).unwrap(), OpenMode::NONE);
    }

    /// The pending hooks answer "nothing" through the app's feed, never a
    /// guess (M02).
    // Covers: specs/client/model.md §9 r4
    #[test]
    fn pending_hooks_answer_nothing() {
        let mut feed = ModelFeed::<NoFeed>::default();
        let w = ClientWorld::default();
        assert!(w.active_rooms.is_none());
        assert!(feed.light(&w).unwrap().is_none());
        assert!(feed.weather_frame(&w).unwrap().is_none());
        assert!(feed.near_rooms(&w).unwrap().is_none());
        assert!(feed
            .map_tiles(
                &w,
                &ViewAssets::new(crate::app::play::unspecified_palette())
            )
            .unwrap()
            .is_empty());
        assert!(feed.player_seed(&w).is_err());
        assert_eq!(PENDING.len(), 5);
    }

    // Covers: specs/client/model.md §1 r2
    #[test]
    fn unit_positions_by_path_kind() {
        let feed = ModelFeed::<NoFeed>::default();
        assert_eq!(
            feed.unit_position(&unit(MONSTER, Some((0x121A, 0x11B9))))
                .unwrap(),
            UnitPosition::Moving {
                x16: 0x121A_8000,
                y16: 0x11B9_8000
            }
        );
        assert_eq!(
            feed.unit_position(&unit(OBJECT, Some((0x1214, 0x11C0))))
                .unwrap(),
            UnitPosition::Static {
                sx: 0x1214,
                sy: 0x11C0
            }
        );
        assert!(feed.unit_position(&unit(ITEM, None)).is_err());
    }
}
