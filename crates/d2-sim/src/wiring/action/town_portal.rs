// Spec: specs/world/objects-2.md §25 (portal pair `0x0056D130`, `0x0056CF40`), §27 (Town Portal cast `0x005BE290`, closing, messages); specs/world/quests-helpers.md §7; specs/world/quests-act2.md §8.11, §8.12 r6
//! Portal pairs and the Town Portal cast.
//!
//! [`View::create_portal_pair`] is `0x0056D130` for every caller (the
//! cast, the quest portals, Tyrael's): object 1 in mode 1 at the caller's
//! spot, object 2 in mode 2 at the destination level's tile-11 spawn
//! point, linked to each other ([`PortalLinks`]: unit +0x94 / +0x98, data
//! +0x18..+0x24). [`View::town_portal_cast`] is word 1 of item-use entry
//! 2 (`items/use.md` §4): the town refusal, the old pair closed, the new
//! one made, its owners and player data +0x48 set.
//!
//! A portal leaves the game through its room's delete list
//! ([`View::remove_portal_object`], `0x0061A270`): the per-client update
//! sends S→C 0x0A to every client whose room adjacency holds that room
//! ([`View::send_room_deletes`], `0x0053A770`).

use std::collections::BTreeMap;

use crate::drlg::{act_of_level, is_town, TOWN_LEVELS};
use crate::game::Game;
use crate::path::coords::Point;
use crate::path::place_seams::{mask, CollisionView};
use crate::units::{ClientId, RoomId, UnitId, UnitType};
use crate::wiring::path::place::{level_spawn, Rooms};

use super::{Pending, View, WiringError};

/// The class of the town portal object (`objects.md` §12).
pub const TOWN_PORTAL_CLASS: u32 = 59;
/// The class of the permanent portal (`objects.md` §12).
const PERMANENT_PORTAL_CLASS: u32 = 60;
/// Free-point size of both ends (`objects-2.md` §25 rules 5, 9, 11, 13).
const PORTAL_SIZE: i32 = 3;
/// Spawn tile index of object 2's destination (§25 rule 9).
const DEST_TILE: u32 = 11;
/// Step of the destination free point (§25 rule 13).
const DEST_STEP: i32 = 5;
/// Tyrael's arrival hook `0x0059DFD0` (§25 rule 11): level 40's tile-12
/// spawn, free point step 7.
const TYRAEL_SOURCE: u32 = 73;
const TYRAEL_TOWN: u32 = 40;
const TYRAEL_TILE: u32 = 12;
const TYRAEL_STEP: i32 = 7;
/// §25 rule 3's exception: class 60 to these levels opens from a town.
const TOWN_OPEN_LEVELS: [u32; 5] = [39, 133, 134, 135, 136];
/// §27.1 step 4: Pandemonium Finale refuses like a town.
const FINALE: u32 = 136;
/// Sound events (`audio/triggers-2.md`): `notintown`,
/// `player_townportal_cast`.
const SOUND_NOT_IN_TOWN: u16 = 24;
const SOUND_CAST: u16 = 7;
/// Unit flag-ex (+0xC8) bit of a linked portal (§25 rule 15).
const FLAG_EX_PARTNER: u32 = 0x400;
/// State 98 and stats 353 / 354 of a linked unit with a stat list (§25
/// rule 15).
const STATE_PARTNER: u16 = 98;
const STAT_PARTNER_TYPE: u16 = 353;
const STAT_PARTNER_GUID: u16 = 354;
/// Unit flags (+0xC4) bit the cast sets (§27.1 step 2).
const UNIT_FLAG_CAST: u32 = 0x40;
/// Fatal asserts of `0x0056D130` (§25 rules 1, 4, 14).
const FATAL_NO_ROOM: u32 = 0xE42;
const FATAL_CROSS_ACT: u32 = 0xE5C;
const FATAL_SOURCE_LEVEL: u32 = 0xE2B;

/// One end's link to the other (§25 rule 15).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PortalLink {
    /// Unit +0x94 / +0x98: the other's type and GUID.
    pub ty: u8,
    pub guid: u32,
    /// Object data +0x20 / +0x24: the other's position.
    pub x: i32,
    pub y: i32,
    /// Object data +0x18 / +0x1C: the tile x / y of the other's room
    /// (none: the other had no room, the fields are not written).
    pub tile: Option<(i32, i32)>,
}

/// The portal links and the players' portal GUIDs.
#[derive(Debug, Clone, Default)]
pub struct PortalLinks {
    /// Per linked portal unit.
    links: BTreeMap<UnitId, PortalLink>,
    /// Player data +0x48 (`0x005353B0` / `0x005353F0`): the GUID of the
    /// player's town portal (object 1 of its last pair). Never cleared
    /// (§27.4).
    player_portal: BTreeMap<UnitId, u32>,
    /// Portals an object call removed, freed when it returns.
    doomed: Vec<UnitId>,
}

impl PortalLinks {
    /// `portal`'s link to its partner.
    pub fn link(&self, portal: UnitId) -> Option<&PortalLink> {
        self.links.get(&portal)
    }

    /// Player data +0x48 of `player` (`None`: never written).
    pub fn player_portal(&self, player: UnitId) -> Option<u32> {
        self.player_portal.get(&player).copied()
    }

    /// Queues `object` for removal once the object call returns
    /// ([`View::flush_portal_removals`]).
    pub(super) fn doom(&mut self, object: UnitId) {
        self.doomed.push(object);
    }
}

impl<X: Pending> View<'_, X> {
    /// `0x00553720(game, O)`: O's partner, the object unit of O +0x94 /
    /// +0x98 (`objects.md` §12 rule 6). `None`: no link, or the partner is
    /// gone. The pair's rooms carry the "has portal" flag, so the
    /// partner's room is never freed under it; it is not streamed here.
    pub fn portal_partner(&self, game: &Game, portal: UnitId) -> Option<UnitId> {
        let l = self.h.portals.link(portal)?;
        let ty = *UnitType::ALL.get(usize::from(l.ty))?;
        if ty != UnitType::Object {
            return None;
        }
        game.lists.find_unit(ty, l.guid)
    }

    /// `0x0056D130(game, owner, room, x, y, level, &out, class, exact)`
    /// (`objects-2.md` §25 rules 1–8): the result (1: both objects
    /// exist) and object 1. `tyrael`: chain 13's record +0x3C is 1 during
    /// the call (Tyrael's portal, `quests-act2.md` §8.11), so the arrival
    /// hook of rule 11 places object 2. Neither end's owner GUID is set
    /// (§25 last paragraph).
    #[allow(clippy::too_many_arguments)]
    pub fn create_portal_pair(
        &mut self,
        game: &mut Game,
        owner: Option<UnitId>,
        room: Option<RoomId>,
        (x, y): (i32, i32),
        level: u32,
        class: u32,
        exact: bool,
        tyrael: bool,
    ) -> (u32, Option<UnitId>) {
        // Rule 1.
        let Some(room) = room else {
            self.h.errors.push(WiringError::Portal(FATAL_NO_ROOM));
            return (0, None);
        };
        let Some(room_level) = self.h.drlg.level_id(game, room) else {
            return (0, None);
        };
        // Rule 3.
        if let Some(o) = owner {
            let open = TOWN_OPEN_LEVELS.contains(&level) && class == PERMANENT_PORTAL_CLASS;
            if is_town(room_level) && !open {
                self.portal_sound(game, o, SOUND_NOT_IN_TOWN, Some(o));
                return (0, None);
            }
        }
        // Rule 4.
        if act_of_level(level) != act_of_level(room_level) {
            self.h.errors.push(WiringError::Portal(FATAL_CROSS_ACT));
            return (0, None);
        }
        // Rule 5.
        let mut p = Point::new(x, y);
        if !exact && !self.portal_spot(room, &mut p) {
            return (0, None);
        }
        let Some(r1) = Rooms(&self.h.drlg).cell_room(Some(room), p.x, p.y) else {
            return (0, None);
        };
        // Rule 6: object 1 (its init sets its destination, §5.5), then
        // the mode set (already mode 1: the update mark only).
        let Some(o1) = self.create_object(game, r1, class, p.x, p.y, 1) else {
            return (0, None);
        };
        self.object_set_mode(game, o1, 1);
        // Rule 7 (none: object 1 is already freed).
        let Some(o2) = self.portal_far_end(game, o1, level, room_level, tyrael) else {
            return (0, None);
        };
        // Rule 8.
        self.or_portal_flags(o2, 3);
        self.h.drlg.refresh_room(game, r1, false);
        if let Some(r2) = game.lists.unit(o2).and_then(|e| e.room()) {
            self.h.drlg.refresh_room(game, r2, false);
        }
        (1, Some(o1))
    }

    /// Rule 5's spot: the field search `0x0064E810(room, &p, origin p,
    /// size 3, 0x3E01, 0xC01, no fallback)`; `false`: none.
    ///
    /// PROVISIONAL (sim/path-placement.md §7.3): a host that has not
    /// loaded `ExpField.D2` (`PathState::field` none, as the play host)
    /// runs the same search without the field test (`0x0064E7B0`).
    fn portal_spot(&self, room: RoomId, p: &mut Point) -> bool {
        let rooms = Rooms(&self.h.drlg);
        let field = self.h.paths.as_ref().and_then(|s| s.field.clone());
        let origin = *p;
        let r = match field.as_deref() {
            Some(f) => crate::path::search::free_point_field(
                &rooms,
                f,
                Some(room),
                p,
                origin,
                PORTAL_SIZE,
                mask::ITEM_FLOOR,
                mask::PORTAL_FIELD,
                false,
            ),
            None => crate::path::search::free_point(
                &rooms,
                Some(room),
                p,
                PORTAL_SIZE,
                mask::ITEM_FLOOR,
                false,
            ),
        };
        matches!(r, Ok(Some(_)))
    }

    /// `0x0056CF40(game, object 1, level, source level)` (§25 rules
    /// 9–15): object 2, linked to object 1; `None` (object 1 removed).
    ///
    /// PROVISIONAL (objects-2.md §25 rules 9, 11): the populate
    /// `0x0052D0F0` of the spawn rooms S and T is left to the next tick's
    /// room pass (the view has no population host); the draws of that
    /// population come one step later than in 1.14d.
    fn portal_far_end(
        &mut self,
        game: &mut Game,
        o1: UnitId,
        level: u32,
        source_level: u32,
        tyrael: bool,
    ) -> Option<UnitId> {
        let act = act_of_level(level);
        // Rules 9–10.
        let Some((s, sx, sy)) = level_spawn(self, game, act, level, DEST_TILE) else {
            self.remove_portal_object(game, o1);
            return None;
        };
        // Rule 11: the arrival hook `0x00545830` (only source level 73
        // with Tyrael's flag runs `0x0059DFD0`).
        let mut p = Point::new(sx, sy);
        let mut r = s;
        let mut hooked = false;
        if source_level == TYRAEL_SOURCE && tyrael {
            let town_act = act_of_level(TYRAEL_TOWN);
            if let Some((t, tx, ty)) = level_spawn(self, game, town_act, TYRAEL_TOWN, TYRAEL_TILE) {
                let mut q = p;
                let found = crate::path::search::free_point_step(
                    &Rooms(&self.h.drlg),
                    Some(t),
                    &mut q,
                    PORTAL_SIZE,
                    mask::PORTAL_DEST,
                    TYRAEL_STEP,
                );
                if let Ok(Some(f)) = found {
                    p = Point::new(tx, ty);
                    r = f;
                    hooked = true;
                }
            }
        }
        // Rule 12.
        let r = if hooked {
            Rooms(&self.h.drlg).cell_room(Some(r), p.x, p.y)
        } else {
            Some(s)
        };
        let r = r.unwrap_or_else(|| {
            p = Point::new(sx, sy);
            s
        });
        // Rule 13: a failed search leaves p as given.
        let found = crate::path::search::free_point_step(
            &Rooms(&self.h.drlg),
            Some(r),
            &mut p,
            PORTAL_SIZE,
            mask::PORTAL_DEST,
            DEST_STEP,
        );
        let r = match found {
            Ok(Some(f)) => f,
            _ => level_spawn(self, game, act, level, DEST_TILE).map_or(s, |(room, _, _)| room),
        };
        // Rule 14.
        let class = self.units.get(o1).map_or(TOWN_PORTAL_CLASS, |u| u.class);
        let Some(o2) = self.create_object(game, r, class, p.x, p.y, 2) else {
            self.remove_portal_object(game, o1);
            return None;
        };
        let src = game
            .lists
            .unit(o1)
            .and_then(|e| e.room())
            .and_then(|room| self.h.drlg.level_id(game, room))
            .unwrap_or(0);
        let Ok(src) = u8::try_from(src) else {
            self.h.errors.push(WiringError::Portal(FATAL_SOURCE_LEVEL));
            return None;
        };
        if let Some(d) = self.object_data_mut(o2) {
            d.interact = src;
            d.portal_flags |= 3;
        }
        self.object_set_mode(game, o2, 2);
        // Rule 15.
        self.link_portals(game, o1, o2);
        Some(o2)
    }

    /// `0x00553590(a, b)` (§25 rule 15): each end's data +0x20 / +0x24 and
    /// +0x18 / +0x1C, then the partner both ways (`0x00621CE0`).
    fn link_portals(&mut self, game: &Game, a: UnitId, b: UnitId) {
        for (u, other) in [(a, b), (b, a)] {
            let Some(e) = game.lists.unit(other) else {
                continue;
            };
            let (ty, guid) = (e.ty as u8, e.guid);
            let (x, y) = self.h.path_position(other);
            let tile = e
                .room()
                .and_then(|r| self.h.drlg.drlg_room(game, r))
                .map(|(d, dr)| (d.room(dr).rect.x, d.room(dr).rect.y));
            self.h.portals.links.insert(
                u,
                PortalLink {
                    ty,
                    guid,
                    x,
                    y,
                    tile,
                },
            );
            if let Some(r) = self.units.get_mut(u) {
                r.flags2 |= FLAG_EX_PARTNER;
            }
            if self.stats.unit_list(u).is_some()
                && usize::from(STATE_PARTNER) < self.stats.data().states.count()
            {
                self.set_state(u, STATE_PARTNER, true);
                self.set_base(u, STAT_PARTNER_TYPE, i32::from(ty));
                self.set_base(u, STAT_PARTNER_GUID, guid as i32);
            }
        }
    }

    /// The Town Portal cast `0x005BE290(game, P; I, …)` (`objects-2.md`
    /// §27.1): the result (1 made, 0 not) and whether the cast's own S→C
    /// 0x7C for the item is due (step 9: not after the refusals of steps
    /// 1 and 4); the caller holds the item and sends it.
    pub fn town_portal_cast(&mut self, game: &mut Game, player: UnitId) -> (u32, bool) {
        // Step 1.
        if self.units.get(player).map(|r| r.ty) != Some(UnitType::Player) {
            return (0, false);
        }
        // Step 2.
        if let Some(r) = self.units.get_mut(player) {
            r.flags |= UNIT_FLAG_CAST;
        }
        // Step 3.
        let room = game.lists.unit(player).and_then(|e| e.room());
        let level = room.and_then(|r| self.h.drlg.level_id(game, r));
        // PROVISIONAL (objects-2.md §27.1 step 3): a player in no room (no
        // level) is refused like a town; the original reads the town test
        // of a null room.
        let Some(level) = level else {
            return (0, false);
        };
        let town = TOWN_LEVELS[usize::from(act_of_level(level))];
        // Step 4.
        if is_town(level) || level == FINALE {
            self.portal_sound(game, player, SOUND_NOT_IN_TOWN, Some(player));
            return (0, false);
        }
        // Step 5.
        self.close_player_portal(game, player);
        // Step 6.
        let (x, y) = self.h.path_position(player);
        let (made, o1) = self.create_portal_pair(
            game,
            Some(player),
            room,
            (x, y),
            town,
            TOWN_PORTAL_CLASS,
            false,
            false,
        );
        // Step 7.
        self.portal_sound(game, player, SOUND_CAST, None);
        // Step 8.
        if made != 0 {
            if let Some(o1) = o1 {
                let pg = game.lists.unit(player).map_or(0, |e| e.guid);
                let g1 = game.lists.unit(o1).map_or(u32::MAX, |e| e.guid);
                self.h.portals.player_portal.insert(player, g1);
                if let Some(d) = self.object_data_mut(o1) {
                    d.owner = Some(pg as i32);
                }
                if let Some(o2) = self.portal_partner(game, o1) {
                    if let Some(d) = self.object_data_mut(o2) {
                        d.owner = Some(pg as i32);
                    }
                    self.h.x.object_portal_opened(o2);
                }
                self.h.x.object_portal_opened(o1);
            }
        }
        // Steps 9–10.
        (made, true)
    }

    /// `0x00535430(game, P)` (`quests-helpers.md` §7): the class-59
    /// object whose GUID is P's player data +0x48 and its partner leave
    /// the game, each after the Act V hook `0x0058CF50`
    /// ([`Pending::object_portal_act5`]). +0x48 stays.
    pub fn close_player_portal(&mut self, game: &mut Game, player: UnitId) {
        let Some(g) = self.h.portals.player_portal(player) else {
            return;
        };
        let Some(o) = game.lists.find_unit(UnitType::Object, g) else {
            return;
        };
        if self.units.get(o).map(|r| r.class) != Some(TOWN_PORTAL_CLASS) {
            return;
        }
        self.h.x.object_portal_act5(o);
        let q = self.portal_partner(game, o);
        self.remove_portal_object(game, o);
        if let Some(q) = q {
            self.h.x.object_portal_act5(q);
            self.remove_portal_object(game, q);
        }
    }

    /// A portal leaves the game (`objects-2.md` §27.4): its room's delete
    /// list gets {type, GUID} (`0x0061A270`), the unit is freed
    /// (`0x00555600`) and the room refreshed (`0x0061AED0(room, 1)`: the
    /// "has portal" flag cleared).
    pub fn remove_portal_object(&mut self, game: &mut Game, object: UnitId) {
        let Some((ty, guid, room)) = game
            .lists
            .unit(object)
            .map(|e| (e.ty as u8, e.guid, e.room()))
        else {
            return;
        };
        if let Some(r) = room {
            self.room_delete(game, r, ty, guid);
        }
        self.h.portals.links.remove(&object);
        self.remove(game, object);
        if let Some(r) = room {
            self.h.drlg.refresh_room(game, r, true);
        }
    }

    /// Frees the portals an object call removed
    /// (`ObjectHost::remove_portal`, `objects.md` §12 rule 12), in order.
    pub fn flush_portal_removals(&mut self, game: &mut Game) {
        for u in std::mem::take(&mut self.h.portals.doomed) {
            self.remove_portal_object(game, u);
        }
    }

    /// `0x0061A270(room, type, GUID)`: a record prepended to the room's
    /// delete list; the act's removal flag (+0x58) set.
    pub fn room_delete(&mut self, game: &mut Game, room: RoomId, ty: u8, guid: u32) {
        self.h
            .room_deletes
            .entry(room)
            .or_default()
            .insert(0, (ty, guid));
        if let Some(a) = game
            .lists
            .room(room)
            .map(|r| r.act)
            .and_then(|act| game.lists.act_mut(act))
        {
            a.pending_removals = true;
        }
    }

    /// `0x0053A770` (`tick.md` §6.5, `objects-2.md` §27.4): for each room
    /// of the client's room adjacency (array order), each delete record
    /// (list order) other than the client's own player: S→C 0x0A.
    pub fn send_room_deletes(&mut self, game: &Game, client: ClientId) {
        let Some(c) = game.lists.client(client) else {
            return;
        };
        let (Some(player), Some(room)) = (c.player, c.room) else {
            return;
        };
        let own = game.lists.unit(player).map(|e| (e.ty as u8, e.guid));
        let adjacent = game
            .lists
            .room(room)
            .map(|r| r.adjacent.clone())
            .unwrap_or_default();
        for r in adjacent {
            let Some(list) = self.h.room_deletes.get(&r).cloned() else {
                continue;
            };
            for (ty, guid) in list {
                if Some((ty, guid)) != own {
                    let m = crate::units::messages::remove_unit(ty, guid);
                    self.h.x.send(player, &m);
                }
            }
        }
    }

    /// The object data of `object` (`None`: no object state or data).
    fn object_data_mut(
        &mut self,
        object: UnitId,
    ) -> Option<&mut crate::world::objects::ObjectData> {
        self.h.objects.as_mut()?.control.data.get_mut(&object)
    }

    /// Object data +0x05 |= `bits`.
    fn or_portal_flags(&mut self, object: UnitId, bits: u8) {
        if let Some(d) = self.object_data_mut(object) {
            d.portal_flags |= bits;
        }
    }

    /// `0x00553380(unit, event, target)`: a sound event on the unit.
    fn portal_sound(&mut self, game: &mut Game, unit: UnitId, event: u16, target: Option<UnitId>) {
        if let Err(e) = crate::units::sound::queue_sound(game, unit, event, target) {
            self.unit_error(crate::game::GameError::from(e).into());
        }
    }
}
