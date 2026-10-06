// Spec: specs/client/model.md (§3 rule 3, §1 rule 2), specs/render/camera.md (§2, §3)
//! [`ModelFeed`]: the [`ViewFeed`] hooks the client world model answers
//! (RW2): the local player's position (camera §3) and unit positions
//! (camera §2). Every other hook (open mode, shake, player seed, unit
//! offsets, map tiles) goes to the wrapped feed, [`NoFeed`] by default,
//! until its owner spec is implemented.
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

use crate::bridge::world::{ClientWorld, ITEM, OBJECT, TILE};
use crate::bridge::ClientUnit;
use crate::rules::{MapTile, OpenMode, UnitPosition, ViewSource};

use super::feed::{NoFeed, RunningShake, ViewFeed};
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
            "unit ({}, {}) has no cell: TODO(spec: msg-stats-items.md open question 3, item placement)",
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

/// The client world model's answers, over `inner` for the rest.
#[derive(Debug, Clone, Default)]
pub struct ModelFeed<F = NoFeed> {
    pub inner: F,
}

impl<F> ModelFeed<F> {
    pub fn new(inner: F) -> Self {
        Self { inner }
    }
}

impl<F: ViewSource> ViewSource for ModelFeed<F> {
    fn unit_position(&self, unit: &ClientUnit) -> Result<UnitPosition, String> {
        unit_position(unit)
    }

    fn unit_offset(&self, unit: &ClientUnit, pose: &UnitPose) -> Result<(i32, i32), String> {
        self.inner.unit_offset(unit, pose)
    }

    fn map_tiles(
        &self,
        world: &ClientWorld,
        assets: &ViewAssets,
    ) -> Result<Vec<MapTile>, ViewError> {
        self.inner.map_tiles(world, assets)
    }
}

impl<F: ViewFeed> ViewFeed for ModelFeed<F> {
    /// `model.md` §3 rule 3: the local player's position; no local player
    /// → `None`.
    fn player(&self, world: &ClientWorld) -> Result<Option<UnitPosition>, ViewError> {
        let Some(unit) = world.local() else {
            return Ok(None);
        };
        unit_position(unit)
            .map(Some)
            .map_err(|message| ViewError::Unresolved {
                what: "local player position",
                spec: "client/model.md",
                message,
            })
    }

    fn open_mode(&self, world: &ClientWorld) -> Result<OpenMode, ViewError> {
        self.inner.open_mode(world)
    }

    fn shake(&self, world: &ClientWorld) -> Result<Option<RunningShake>, ViewError> {
        self.inner.shake(world)
    }

    fn player_seed(&mut self, world: &ClientWorld) -> Result<&mut Seed, ViewError> {
        self.inner.player_seed(world)
    }

    fn blank_screen(&self, world: &ClientWorld) -> Result<bool, ViewError> {
        self.inner.blank_screen(world)
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
        // The other hooks are still the placeholder's.
        assert!(feed.open_mode(&w).is_err());
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
