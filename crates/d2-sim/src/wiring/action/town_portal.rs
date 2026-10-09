// Spec: specs/world/objects.md §5.5, §12; specs/world/objects-2.md §22; specs/skills/bodies-3.md §4.4
//! The Town Portal scroll / tome: the portal object pair and its links.
//!
//! Using a `tsc` / `tbk` item ends in [`View::create_town_portal`]
//! (`0x0056D130`, `objects-2.md` §22: object 1 of class 59 in mode 1 at
//! the player's room, object 2 of the same class in mode 2 at the
//! destination). The portal travel (`objects.md` §12, `misc::portal`)
//! then runs on the objects through the object host's partner, player
//! portal GUID and removal seams, which read [`PortalLinks`] here before
//! they ask [`super::Pending`].
//!
//! PROVISIONAL (REC-117): `0x0056D130`'s body is not specified past the
//! allocation draws (`cube.md` §9 item 4), and the item-use first word
//! of `tsc` / `tbk` (`0x005BF240` table) is unwritten. d2rs puts the
//! field portal at the free spot nearest the player (size 3, mask
//! 0x1C09), the town portal at the act's town spawn point (tile index 11,
//! else 0), links the two, owns both by the player, and sets player data
//! +0x48 to the field portal's GUID, so §12 rule 12 removes the pair
//! when the player arrives from town. A cast in town, or a cast without
//! a town spawn, creates nothing. A new cast removes the player's
//! previous pair.
//!
//! A cast in a town makes nothing: the item-use dispatcher refuses it
//! before any cost (`items/use.md` §4; recorded under Wine in the Rogue
//! Encampment, also with a field pair open: `3F FF <item> FF FF` and
//! `7C 04 <item>` only; `facts/items/a1-town-portal-cold-plains.tsv`
//! frame 362; q-fix-real-tp-town-cast, REC-243 (1) withdrawn).
// d2rs-own, unverified

use std::collections::BTreeMap;

use crate::game::Game;
use crate::units::{RoomId, UnitId};

use super::{Pending, View};

/// The class of the town portal object (`objects.md` §12).
pub const TOWN_PORTAL_CLASS: u32 = 59;
/// The footprint mask and size of a portal spot (`objects.md` §12 r10).
const SPOT_MASK: u32 = 0x1C09;
const SPOT_SIZE: i32 = 3;
/// Spawn tile index of `0x0056CF40`'s destination (`population.md` §1 r2).
const PORTAL_TILE: u32 = 11;

/// Which portal pairs exist and whose they are.
#[derive(Debug, Clone, Default)]
pub struct PortalLinks {
    /// Both directions of every pair.
    partner: BTreeMap<UnitId, UnitId>,
    /// The player's field portal (player data +0x48 holds its GUID).
    field: BTreeMap<UnitId, UnitId>,
    /// Portals the object call removed, freed when it returns.
    doomed: Vec<UnitId>,
}

impl PortalLinks {
    /// `0x00553720`: the partner portal of `object`.
    pub fn partner(&self, object: UnitId) -> Option<UnitId> {
        self.partner.get(&object).copied()
    }

    /// The field portal of `player`'s pair.
    pub fn field_portal(&self, player: UnitId) -> Option<UnitId> {
        self.field.get(&player).copied()
    }

    fn link(&mut self, player: UnitId, field: UnitId, town: UnitId) {
        self.partner.insert(field, town);
        self.partner.insert(town, field);
        self.field.insert(player, field);
    }

    /// Queues `object` for removal (§12 rule 12) and forgets it as a
    /// member of a pair (its partner keeps its own links until its own
    /// removal).
    pub(super) fn doom(&mut self, object: UnitId) {
        self.partner.remove(&object);
        self.field.retain(|_, f| *f != object);
        self.doomed.push(object);
    }

    /// Forgets `object` and its partner; the partner (if any) is returned.
    fn unlink(&mut self, object: UnitId) -> Option<UnitId> {
        let other = self.partner.remove(&object)?;
        self.partner.remove(&other);
        self.field.retain(|_, f| *f != object && *f != other);
        Some(other)
    }
}

impl<X: Pending> View<'_, X> {
    /// `0x0056D130` for the Town Portal scroll or tome of `player`: the
    /// pair of portal objects. `None`: nothing was created (module docs).
    pub fn create_town_portal(
        &mut self,
        game: &mut Game,
        player: UnitId,
    ) -> Option<(UnitId, UnitId)> {
        self.h.objects.as_ref()?;
        let room = game.lists.unit(player)?.room()?;
        let level = self.h.drlg.level_id(game, room)?;
        let guid = game.lists.unit(player)?.guid;
        // A town cast is refused by the item-use dispatcher before any
        // cost; this is its belt (module docs).
        if crate::drlg::is_town(level) {
            return None;
        }
        let far_level = crate::drlg::TOWN_LEVELS[usize::from(crate::drlg::act_of_level(level))];
        if let Some(old) = self.h.portals.field_portal(player) {
            self.remove_portal_pair(game, old, Some(player));
        }
        let (px, py) = self.h.path_position(player);
        let (nroom, nx, ny) = self.portal_spot(room, px, py)?;
        let (froom, fx, fy) = self.town_spot(game, far_level)?;
        // `near` (the field portal) is next to the player, `far` in town.
        let near = self.create_object(game, nroom, TOWN_PORTAL_CLASS, nx, ny, 1)?;
        let Some(far) = self.create_object(game, froom, TOWN_PORTAL_CLASS, fx, fy, 2) else {
            self.remove_and_tell(game, near, None);
            return None;
        };
        let (field, dest) = (near, far);
        self.h.portals.link(player, field, dest);
        let st = self.h.objects.as_mut()?;
        for (portal, to) in [(near, far_level), (far, level)] {
            if let Some(d) = st.control.data.get_mut(&portal) {
                d.interact = u8::try_from(to).unwrap_or(u8::MAX);
                d.owner = Some(guid as i32);
            }
        }
        // The field portal appears in the player's room: announce it
        // (the town one comes with the room switch).
        self.add_messages(game, player, near);
        Some((field, dest))
    }

    /// `0x0056D130` for a quest portal (Tyrael's, `quests-act2.md` §8.11):
    /// one portal object of `class` in mode 1 at the free spot nearest
    /// (x, y) in `player`'s room, to `level`, owned by the player.
    /// PROVISIONAL (REC-174): no partner object at the destination; the
    /// spot rules are the town portal's. `// d2rs-own, unverified`.
    pub fn create_quest_portal(
        &mut self,
        game: &mut Game,
        player: UnitId,
        (x, y): (i32, i32),
        class: u32,
        level: u32,
    ) -> Option<UnitId> {
        self.h.objects.as_ref()?;
        let room = game.lists.unit(player)?.room()?;
        let guid = game.lists.unit(player)?.guid;
        let (froom, fx, fy) = self.portal_spot(room, x, y)?;
        let portal = self.create_object(game, froom, class, fx, fy, 1)?;
        let st = self.h.objects.as_mut()?;
        if let Some(d) = st.control.data.get_mut(&portal) {
            d.interact = u8::try_from(level).unwrap_or(u8::MAX);
            d.owner = Some(guid as i32);
        }
        self.add_messages(game, player, portal);
        Some(portal)
    }

    /// `objects.md` §12 rule 12's removal of `object` and its partner:
    /// `0x00555600` on each, the links forgotten.
    pub fn remove_portal_pair(&mut self, game: &mut Game, object: UnitId, notify: Option<UnitId>) {
        let other = self.h.portals.unlink(object);
        for u in std::iter::once(object).chain(other) {
            self.remove_and_tell(game, u, notify);
        }
    }

    /// Frees the portals an object call removed (`ObjectHost::remove_portal`).
    ///
    /// PROVISIONAL (REC-117): the original's removal `0x0061A270(room, 2,
    /// GUID)` tells the clients in the room; here `notify` (the operator)
    /// gets S→C 0x0A for each unit it removes.
    pub fn flush_portal_removals(&mut self, game: &mut Game, notify: Option<UnitId>) {
        for u in std::mem::take(&mut self.h.portals.doomed) {
            self.remove_and_tell(game, u, notify);
        }
    }

    fn remove_and_tell(&mut self, game: &mut Game, u: UnitId, notify: Option<UnitId>) {
        let Some((ty, guid)) = game.lists.unit(u).map(|e| (e.ty as u8, e.guid)) else {
            return;
        };
        if let Some(p) = notify {
            self.h
                .x
                .send(p, &crate::units::messages::remove_unit(ty, guid));
        }
        self.remove(game, u);
    }

    /// The free spot for a portal footprint around (x, y) in `room`.
    fn portal_spot(&self, room: RoomId, x: i32, y: i32) -> Option<(RoomId, i32, i32)> {
        let mut p = crate::path::coords::Point::new(x, y);
        let rooms = crate::wiring::path::place::Rooms(&self.h.drlg);
        match crate::path::search::free_point(
            &rooms,
            Some(room),
            &mut p,
            SPOT_SIZE,
            SPOT_MASK,
            false,
        ) {
            Ok(Some(r)) => Some((r, p.x, p.y)),
            _ => None,
        }
    }

    /// The town's spawn point (`0x0061B060`, tile index 11, else 0), then
    /// the portal spot around it.
    fn town_spot(&mut self, game: &mut Game, town: u32) -> Option<(RoomId, i32, i32)> {
        let act = crate::drlg::act_of_level(town);
        let (room, x, y) = [PORTAL_TILE, 0]
            .into_iter()
            .find_map(|t| crate::wiring::path::place::level_spawn(self, game, act, town, t))?;
        self.portal_spot(room, x, y)
    }
}
