// Spec: specs/world/quests.md §9; specs/world/quests-helpers.md §4.2, §5, §7; specs/world/quests-act3.md §6; specs/world/vendors-2.md §10.1; specs/world/quests-act1.md §10; specs/world/quests-act1-rest.md §1–§3; specs/world/quests-act2.md §1.5; specs/world/objects.md §3, §4, §7; specs/sim/tick.md §5.2
//! [`HostQuests`]: the quests' world on the wired host. Every
//! [`QuestWorld`] call goes to [`EconomyQuests`] (the economy plus the
//! rest), except those the action wiring provides:
//!
//! - the object calls on an object with object data (the object state,
//!   `wiring::action::objects`): its mode (unit +0x10), the mode set
//!   `0x00624690` ([`View::object_set_mode`]), the animation length
//!   `0x00640E90` (`objects.txt` +0xDC `FrameCnt1`, raw), object events
//!   through the timer queue `0x005417D0` (event 7 for the quest event),
//!   the Act I object allocation `0x00555230` with a mode
//!   ([`View::create_object`]) and the collision free `0x00623830` (the
//!   object code's footprint free, `View::free_object_footprint`);
//! - the level of a unit in a DRLG room (`DrlgWorld::level_id`);
//! - `0x006280D0(item, 0x10)` on an item of the game's item store;
//! - `missiles.txt` `Range` from the action tables;
//! - the chest treasure `0x00585B90(op, kind)` on the object drop state
//!   (`ActionHooks::object_drops`, [`super::object_chest_drop`]);
//! - the quest helpers' host calls: the path target `0x0056D2C0` (path
//!   provider), the trade button `0x00568060` (`vendors-2.md` §10.1), the
//!   obelisk's 0x44 cancel, the town portal GUID / partner / removal
//!   (the action wiring's portal seams, one home with `objects.md` §12),
//!   the mode request `0x005DDFC0` (the monster mode change), the item
//!   level `0x00558200`;
//!
//! and those of the host parts a caller lends ([`HostQuests::inventory`]:
//! the reward `0x005466B0`, [`super::quest_reward`], and the cube close's
//! `0x0055FA40`; [`HostQuests::chats`]: the chat-node frees `0x00572E00`,
//! `0x00573180`).
//!
//! Left to the rest: the Steeg Stone release `0x00584820` (no object data
//! +0 for class 337 in d2-sim: no object spec states that object).
//!
//! An object without object data, a unit outside a DRLG room and an item
//! outside the store keep the rest's answer, as before.
//!
//! Status: wired, unverified.

use crate::drlg::TileRect;
use crate::missiles::{self, MissileParams};
use crate::path::place_seams::CollisionView;
use crate::rng::Seed;
use crate::units::{RoomId, UnitId, UnitType};
use crate::wiring::action::{ActionHooks, Pending, View};
use crate::wiring::path::place::Rooms;
use crate::world::quests::helpers::{self, QuestMissile};
use crate::world::quests::{PlayerQuests, QuestChain, QuestWorld, UnitKind};

use super::quest_reward::{self, Placed, QuestInventory};
use super::{EconomyQuests, QuestRest};
use crate::items::moves::Spot;
use crate::monsters::ai::seams::{AiModes, ModeTarget};
use crate::world::cube::trade_action;
use crate::world::npc::InteractionList;
use crate::world::quests::act2;
use std::collections::BTreeMap;

/// [`EconomyQuests`] on the action wiring's hooks, with the calls the
/// action wiring provides answered there (module doc), and the host
/// parts a caller may lend for the call.
pub struct HostQuests<'e, 'a, X, R> {
    pub inner: EconomyQuests<'e, 'a, ActionHooks<X>, R>,
    /// The host's inventory model (the reward `0x005466B0`, the cube
    /// close's `0x0055FA40`); `None`: the rest's answers.
    pub inventory: Option<&'e mut dyn QuestInventory<ActionHooks<X>>>,
    /// The NPCs' interaction lists (monster data +0x30, `npc.md` §2;
    /// `InteractionState::lists`); `None`: the chat-node calls are
    /// reported unhandled.
    pub chats: Option<&'e mut BTreeMap<UnitId, InteractionList>>,
}

impl<'e, 'a, X: Pending, R: QuestRest> HostQuests<'e, 'a, X, R> {
    pub fn new(inner: EconomyQuests<'e, 'a, ActionHooks<X>, R>) -> Self {
        Self {
            inner,
            inventory: None,
            chats: None,
        }
    }

    /// The object has object data (a created object state knows it).
    fn known(&self, object: UnitId) -> bool {
        self.inner
            .econ
            .hooks
            .objects
            .as_ref()
            .is_some_and(|s| s.control.data.contains_key(&object))
    }

    /// The active room has a DRLG room on this host.
    fn drlg_room(&self, room: RoomId) -> bool {
        let e = &*self.inner.econ;
        e.hooks.drlg.drlg_room(e.game, room).is_some()
    }

    /// Runs `f` on the action wiring's view over the economy's parts.
    /// Runs a drop helper (`objects-2.md` §20) with the game's drop state
    /// (`ActionHooks::object_drops`) lent out and the action tables'
    /// `levels`; the economy's item store, game seed and unique bits go
    /// back to the hooks for the call (as [`Self::object_treasure`]).
    /// `None`: no drop state.
    fn with_drop_state<T>(
        &mut self,
        f: impl FnOnce(
            &mut crate::wiring::action::ActionHooks<X>,
            &mut crate::units::hooks::Sim<'_>,
            &mut super::DeathDrops,
            &[d2_data::tables::Levels],
            &mut super::NoSpot,
        ) -> T,
    ) -> Option<T> {
        let e = &mut *self.inner.econ;
        let mut d = e.hooks.object_drops.take()?;
        let at = e.hooks.tables.clone();
        std::mem::swap(&mut e.hooks.items, &mut *e.items);
        e.hooks.game_seed = e.fields.seed;
        e.hooks.uniques = std::mem::take(&mut e.fields.uniques);
        let out = {
            let mut sim = crate::units::hooks::Sim {
                game: &mut *e.game,
                units: &mut *e.units,
                stats: &mut *e.stats,
                data: e.data,
            };
            f(
                &mut *e.hooks,
                &mut sim,
                &mut d,
                &at.levels,
                &mut super::NoSpot,
            )
        };
        std::mem::swap(&mut e.hooks.items, &mut *e.items);
        e.fields.seed = e.hooks.game_seed;
        e.fields.uniques = std::mem::take(&mut e.hooks.uniques);
        e.hooks.object_drops = Some(d);
        Some(out)
    }

    fn view<T>(&mut self, f: impl FnOnce(&mut crate::game::Game, &mut View<'_, X>) -> T) -> T {
        let e = &mut *self.inner.econ;
        let mut v = View::of(&mut *e.units, &mut *e.stats, e.data, &mut *e.hooks);
        f(&mut *e.game, &mut v)
    }

    /// `0x00559A30` with the drop code `code` (`treasure.md` §9,
    /// [`super::unit_quest_drop`]) when the game holds the drop state
    /// (`ActionHooks::object_drops`) and `unit` has a record; `None`: no
    /// drop state (the caller keeps the rest's answer).
    fn econ_quest_drop(
        &mut self,
        unit: UnitId,
        code: [u8; 4],
        quality: u8,
        p7: i32,
    ) -> Option<Option<UnitId>> {
        self.inner.econ.units.get(unit)?;
        self.with_drop_state(|h, sim, d, _, spots| {
            super::unit_quest_drop(h, sim, d, spots, unit, Some(code), quality, -1, p7)
        })
    }

    /// A monster of `class` allocated in `room` at (x, y) in `mode`
    /// (`units.md` §3.1); NPC classes count as allied.
    fn spawn_unit(&mut self, room: RoomId, x: i32, y: i32, class: u16, mode: u8) -> Option<UnitId> {
        let allied = self
            .inner
            .econ
            .hooks
            .tables
            .combat
            .monstats
            .get(usize::from(class))
            .is_some_and(|m| m.npc);
        let req = crate::units::lifecycle::AllocRequest {
            ty: UnitType::Monster,
            class: u32::from(class),
            room: Some(room),
            add: true,
            fixed_guid: None,
            mode: u32::from(mode),
            allied,
        };
        self.view(|g, v| v.allocate(g, &req, x, y))
    }

    /// `0x005417D0` on the game's timer queue (`tick.md` §5.2); a refused
    /// schedule is a unit error of the action wiring.
    fn schedule(&mut self, object: UnitId, ev: u8, frame: i32) {
        let e = &mut *self.inner.econ;
        if let Err(err) = e
            .game
            .schedule_event(object, u32::from(ev), frame, None, 0, 0)
        {
            let mut v = View::of(&mut *e.units, &mut *e.stats, e.data, &mut *e.hooks);
            v.unit_error(err.into());
        }
    }
}

impl<X: Pending, R: QuestRest> HostQuests<'_, '_, X, R> {
    /// `0x005466B0` (`quests.md` §9.1) on the lent inventory model
    /// ([`quest_reward`]): create, place, else drop next to the player
    /// when droppable, else free.
    fn reward_on_host(
        &mut self,
        player: UnitId,
        code: [u8; 4],
        level: i32,
        quality: u8,
        droppable: bool,
    ) -> Option<UnitId> {
        let made = quest_reward::create_reward(&mut *self.inner.econ, player, code, level, quality);
        let item = match made {
            Ok(Some(item)) => item,
            Ok(None) => return None,
            Err(e) => {
                if let Some(inv) = self.inventory.as_deref_mut() {
                    inv.fault(e);
                }
                return None;
            }
        };
        let inv = self.inventory.as_deref_mut()?;
        if let Placed::Stored(item) =
            quest_reward::place_reward(&mut *self.inner.econ, inv, player, item)
        {
            return Some(item);
        }
        let spot = if droppable {
            self.reward_spot(player)
        } else {
            None
        };
        let inv = self.inventory.as_deref_mut()?;
        quest_reward::drop_or_free(&mut *self.inner.econ, inv, item, spot)
    }

    /// §9.1's drop spot: `0x00545340` ([`helpers::free_spot`]) from the
    /// player's path position and room, size 1, mask 0x3E01, limit 100.
    /// `None`: the player has no position (the reward is freed).
    ///
    /// PROVISIONAL (quests.md §9.1; REC-none): when the search accepts
    /// nothing (or the player's room has no DRLG room here) the item is
    /// dropped at the player's own position and room (`0x00545340` leaves
    /// the point as passed; what the drop does with its null out room is
    /// not written).
    fn reward_spot(&mut self, player: UnitId) -> Option<Spot> {
        let (x, y, room) = self.unit_position(player)?;
        let found = if self.drlg_room(room) {
            helpers::free_spot(
                self,
                room,
                x,
                y,
                quest_reward::DROP_SIZE,
                quest_reward::DROP_MASK,
                quest_reward::DROP_LIMIT,
            )
        } else {
            None
        };
        let (x, y, room) = found.unwrap_or((x, y, room));
        Some(Spot { room, x, y })
    }
}

impl<X: Pending, R: QuestRest> QuestWorld for HostQuests<'_, '_, X, R> {
    /// Unit +0x10 of an object with object data.
    fn object_mode(&self, object: UnitId) -> i32 {
        if self.known(object) {
            return self
                .inner
                .econ
                .units
                .get(object)
                .map_or(0, |r| r.mode as i32);
        }
        self.inner.object_mode(object)
    }
    /// `0x00624690` (`objects.md` §4) on an object with object data.
    fn set_object_mode(&mut self, object: UnitId, mode: i32) {
        if let (true, Ok(m)) = (self.known(object), u8::try_from(mode)) {
            if self.view(|g, v| v.object_set_mode(g, object, m)) {
                return;
            }
        }
        self.inner.set_object_mode(object, mode)
    }
    /// `0x00640E90`: the class's `objects.txt` record, +0xDC (`FrameCnt1`
    /// as stored, × 256).
    fn object_anim_length(&self, object: UnitId) -> i32 {
        if let Some(st) = self.inner.econ.hooks.objects.as_ref() {
            if let Some(d) = st.control.data.get(&object) {
                if let Ok(o) = st.tables.object(d.class) {
                    return o.framecnt1 as i32;
                }
            }
        }
        self.inner.object_anim_length(object)
    }
    fn schedule_object_event(&mut self, object: UnitId, ev: u8, frame: i32) {
        if self.known(object) {
            return self.schedule(object, ev, frame);
        }
        self.inner.schedule_object_event(object, ev, frame)
    }
    /// Object event 7 (QUESTFN, `units.md` §6.4).
    fn schedule_quest_event(&mut self, object: UnitId, frame: i32) {
        if self.known(object) {
            return self.schedule(object, 7, frame);
        }
        self.inner.schedule_quest_event(object, frame)
    }
    /// `0x00555230(game, 2, class, …, mode)` through the object state
    /// ([`View::create_object`]: allocation and the §3 init); no object
    /// state: the rest's.
    fn spawn_object(
        &mut self,
        room: RoomId,
        x: i32,
        y: i32,
        class: u16,
        mode: i32,
    ) -> Option<UnitId> {
        if let (true, Ok(m)) = (self.inner.econ.hooks.objects.is_some(), u8::try_from(mode)) {
            return self.view(|g, v| v.create_object(g, room, u32::from(class), x, y, m));
        }
        self.inner.spawn_object(room, x, y, class, mode)
    }
    /// `0x00623830`: the object code's footprint free
    /// (`View::free_object_footprint`, `objects.md` §8.2, §10).
    fn free_object_collision(&mut self, object: UnitId) {
        if self.known(object) {
            self.view(|g, v| v.free_object_footprint(g, object));
            return;
        }
        self.inner.free_object_collision(object)
    }
    /// The room's level when the unit is in a DRLG room.
    fn unit_level(&self, unit: UnitId) -> Option<u32> {
        let e = &*self.inner.econ;
        let room = e.game.lists.unit(unit).and_then(|u| u.room());
        room.and_then(|r| e.hooks.drlg.level_id(e.game, r))
            .or_else(|| self.inner.unit_level(unit))
    }
    fn interact_unit(&mut self, player: UnitId) -> Option<(u8, u32)> {
        self.inner.interact_unit(player)
    }
    fn set_interact_unit(&mut self, player: UnitId, unit: Option<(u8, u32)>) {
        self.inner.set_interact_unit(player, unit)
    }
    /// `0x006280D0(item, 0x10)`: item flags +0x18 |= identified.
    fn identify_item(&mut self, item: UnitId) {
        match self.inner.econ.items.get_mut(item) {
            Some(i) => i.flags |= crate::items::flag::IDENTIFIED,
            None => self.inner.identify_item(item),
        }
    }

    fn frame(&self) -> i32 {
        self.inner.frame()
    }
    fn difficulty(&self) -> u8 {
        self.inner.difficulty()
    }
    fn expansion(&self) -> bool {
        self.inner.expansion()
    }
    fn game_type(&self) -> u8 {
        self.inner.game_type()
    }
    fn has_act2(&self) -> bool {
        self.inner.has_act2()
    }
    fn players(&self) -> Vec<UnitId> {
        self.inner.players()
    }
    fn first_client_player(&self) -> Option<UnitId> {
        self.inner.first_client_player()
    }
    fn guid(&self, unit: UnitId) -> u32 {
        self.inner.guid(unit)
    }
    fn player_by_guid(&self, guid: u32) -> Option<UnitId> {
        self.inner.player_by_guid(guid)
    }
    fn quests(&mut self, player: UnitId) -> Option<&mut PlayerQuests> {
        self.inner.quests(player)
    }
    fn unit_act(&self, unit: UnitId) -> Option<u8> {
        self.inner.unit_act(unit)
    }
    fn player_class(&self, player: UnitId) -> u8 {
        self.inner.player_class(player)
    }
    fn unit_seed(&mut self, unit: UnitId) -> &mut Seed {
        self.inner.unit_seed(unit)
    }
    fn stat(&self, unit: UnitId, stat: u16) -> i32 {
        self.inner.stat(unit, stat)
    }
    fn base_stat(&self, unit: UnitId, stat: u16) -> i32 {
        self.inner.base_stat(unit, stat)
    }
    fn add_stat(&mut self, unit: UnitId, stat: u16, delta: i32) {
        self.inner.add_stat(unit, stat, delta)
    }
    fn attach_sound(&mut self, player: UnitId, sound: u16) {
        self.inner.attach_sound(player, sound)
    }
    fn player_byte_4c(&self, player: UnitId) -> u8 {
        self.inner.player_byte_4c(player)
    }
    fn set_player_byte_4c(&mut self, player: UnitId, v: u8) {
        self.inner.set_player_byte_4c(player, v)
    }
    fn quest_chain(&mut self, unit: UnitId) -> Option<&mut QuestChain> {
        self.inner.quest_chain(unit)
    }
    fn unit_kind(&self, unit: UnitId) -> UnitKind {
        self.inner.unit_kind(unit)
    }
    fn monster_by_guid(&self, guid: u32) -> Option<(UnitId, u16)> {
        self.inner.monster_by_guid(guid)
    }
    fn monster_class(&self, unit: UnitId) -> Option<u16> {
        self.inner.monster_class(unit)
    }
    fn players_near(&self, unit: UnitId) -> Vec<UnitId> {
        self.inner.players_near(unit)
    }
    fn party_members(&self, player: UnitId) -> Option<Vec<UnitId>> {
        self.inner.party_members(player)
    }
    fn send(&mut self, player: UnitId, msg: &[u8]) {
        self.inner.send(player, msg)
    }
    fn send_text_list(&mut self, player: UnitId, npc: UnitId, list: &[(u16, u32)]) {
        self.inner.send_text_list(player, npc, list)
    }
    fn has_item(&self, player: UnitId, code: [u8; 4]) -> bool {
        self.inner.has_item(player, code)
    }
    fn item_code(&self, item: UnitId) -> Option<[u8; 4]> {
        self.inner.item_code(item)
    }
    fn delete_item(&mut self, player: UnitId, code: [u8; 4]) {
        self.inner.delete_item(player, code)
    }
    fn reward_item(
        &mut self,
        player: UnitId,
        code: [u8; 4],
        level: i32,
        quality: u8,
        droppable: bool,
    ) -> Option<UnitId> {
        if self.inventory.is_none() {
            return self
                .inner
                .reward_item(player, code, level, quality, droppable);
        }
        self.reward_on_host(player, code, level, quality, droppable)
    }
    /// `0x00559A30(game, unit, quality, &out, 0, −1, 0)` with the drop
    /// code `code` ([`Self::econ_quest_drop`]); without the drop state,
    /// the rest's answer.
    fn drop_item_at(&mut self, unit: UnitId, code: [u8; 4], quality: u8) -> bool {
        match self.econ_quest_drop(unit, code, quality, 0) {
            Some(item) => item.is_some(),
            None => self.inner.drop_item_at(unit, code, quality),
        }
    }
    fn quest_items(&self, player: UnitId) -> Vec<(UnitId, u8)> {
        self.inner.quest_items(player)
    }
    fn den_region(&self) -> (u32, u32, u32, u32) {
        self.inner.den_region()
    }
    fn true_tomb_level(&self) -> u32 {
        self.inner.true_tomb_level()
    }
    /// `0x00545340` from the player's path position and room
    /// ([`helpers::free_spot`]) when the player is in a DRLG room; else
    /// the rest's.
    fn free_spot(
        &mut self,
        player: UnitId,
        size: u32,
        mask: u32,
        radius: u32,
        limit: u32,
    ) -> Option<(i32, i32)> {
        if let Some((x, y, room)) = self.unit_position(player) {
            if self.drlg_room(room) {
                return helpers::free_spot(self, room, x, y, size, mask, limit).map(|p| (p.0, p.1));
            }
        }
        self.inner.free_spot(player, size, mask, radius, limit)
    }
    fn create_portal(&mut self, player: UnitId, x: i32, y: i32, class: u16, level: u32) -> bool {
        self.inner.create_portal(player, x, y, class, level)
    }
    fn object_by_guid(&self, guid: u32) -> Option<(UnitId, u16)> {
        self.inner.object_by_guid(guid)
    }
    fn mercenary_reward(&mut self, player: UnitId, npc: u16) {
        self.inner.mercenary_reward(player, npc)
    }
    /// `0x00620870` on a unit of the game's lists: its path position and
    /// its list room (an object's static path +0x00; `None` for a unit
    /// left in a freed room, `quests-act1-rest.md` §9 item 2).
    fn unit_position(&self, unit: UnitId) -> Option<(i32, i32, RoomId)> {
        let e = &*self.inner.econ;
        match e.game.lists.unit(unit) {
            Some(u) => {
                let room = u.room()?;
                let (x, y) = e.hooks.path_position(unit);
                Some((x, y, room))
            }
            None => self.inner.unit_position(unit),
        }
    }
    /// The path position of a unit of the game's lists, with or without
    /// a room.
    fn unit_xy(&self, unit: UnitId) -> Option<(i32, i32)> {
        let e = &*self.inner.econ;
        match e.game.lists.unit(unit) {
            Some(_) => Some(e.hooks.path_position(unit)),
            None => self.inner.unit_position(unit).map(|(x, y, _)| (x, y)),
        }
    }
    /// `0x00619730`: the active room's sub-tile box, the last row and
    /// column excluded.
    fn room_contains(&self, room: RoomId, x: i32, y: i32) -> bool {
        let e = &*self.inner.econ;
        match e.hooks.drlg.subtiles(e.game, room) {
            Some(t) => t.contains(x, y),
            None => self.inner.room_contains(room, x, y),
        }
    }
    /// `0x00463740` (`DrlgWorld::find_room`) from a room with a DRLG room.
    fn room_at(&self, room: RoomId, x: i32, y: i32) -> Option<RoomId> {
        let e = &*self.inner.econ;
        if e.hooks.drlg.drlg_room(e.game, room).is_some() {
            return e.hooks.drlg.find_room(e.game, room, x, y);
        }
        self.inner.room_at(room, x, y)
    }
    /// `0x00545340` ([`helpers::free_spot`]) from a DRLG room; else the
    /// rest's.
    #[allow(clippy::too_many_arguments)]
    fn free_spot_at(
        &mut self,
        room: RoomId,
        x: i32,
        y: i32,
        size: u32,
        mask: u32,
        radius: u32,
        limit: u32,
    ) -> Option<(i32, i32, RoomId)> {
        if self.drlg_room(room) {
            return helpers::free_spot(self, room, x, y, size, mask, limit);
        }
        self.inner
            .free_spot_at(room, x, y, size, mask, radius, limit)
    }
    /// `0x005B2F20`: a monster unit allocated in a DRLG room through the
    /// action wiring (`units.md` §3.1); else the rest's.
    fn spawn_monster(
        &mut self,
        room: RoomId,
        x: i32,
        y: i32,
        class: u16,
        mode: u8,
        r: u32,
    ) -> Option<UnitId> {
        if self.drlg_room(room) {
            return self.spawn_unit(room, x, y, class, mode);
        }
        self.inner.spawn_monster(room, x, y, class, mode, r)
    }
    fn or_unit_flags(&mut self, unit: UnitId, flags: u32) {
        self.inner.or_unit_flags(unit, flags)
    }
    fn monsters(&self) -> Vec<UnitId> {
        self.inner.monsters()
    }
    fn npc_chat_clients(&self, npc: UnitId) -> Option<Vec<UnitId>> {
        self.inner.npc_chat_clients(npc)
    }
    /// `0x005A7E60` + `0x005A7C20` (removal mode): the monster of the
    /// game's lists is removed at once and every player told
    /// (PROVISIONAL, REC-127: the removal mode's animation is not
    /// modelled); else the rest's.
    fn remove_monster(&mut self, monster: UnitId) {
        let e = &mut *self.inner.econ;
        let Some((ty, guid)) = e.game.lists.unit(monster).map(|u| (u.ty as u8, u.guid)) else {
            return self.inner.remove_monster(monster);
        };
        let msg = crate::units::messages::remove_unit(ty, guid);
        for p in e.game.lists.units_of_type(UnitType::Player) {
            e.hooks.x.send(p, &msg);
        }
        self.view(|g, v| v.remove(g, monster));
    }
    fn drop_preset_monster(&mut self, act: u8, class: u16) {
        self.inner.drop_preset_monster(act, class)
    }
    /// The first object of `class` in `object`'s act (PROVISIONAL,
    /// REC-127: the spec's "rooms of the object's room list" is the
    /// whole act here); else the rest's.
    fn find_object_near(&self, object: UnitId, class: u16) -> Option<UnitId> {
        let e = &*self.inner.econ;
        let act_of = |u: UnitId| {
            e.game
                .lists
                .unit(u)
                .and_then(|u| u.room())
                .and_then(|r| e.game.lists.room(r))
                .map(|r| r.act)
        };
        let Some(act) = act_of(object) else {
            return self.inner.find_object_near(object, class);
        };
        e.game
            .lists
            .units_of_type(UnitType::Object)
            .into_iter()
            .filter(|&u| e.units.get(u).is_some_and(|r| r.class == u32::from(class)))
            .find(|&u| act_of(u) == Some(act))
    }
    fn create_object(&mut self, room: RoomId, x: i32, y: i32, class: u16) -> Option<UnitId> {
        if self.inner.econ.hooks.objects.is_some() && self.drlg_room(room) {
            return self.view(|g, v| v.create_object(g, room, u32::from(class), x, y, 0));
        }
        self.inner.create_object(room, x, y, class)
    }
    fn open_quest_message(&mut self, player: UnitId, object: UnitId, msg: u16) {
        self.inner.open_quest_message(player, object, msg)
    }
    /// [`Self::spawn_monster`] with the spawn flags (PROVISIONAL, REC-127:
    /// spread and flags are not applied).
    #[allow(clippy::too_many_arguments)]
    fn spawn_monster_flags(
        &mut self,
        room: RoomId,
        x: i32,
        y: i32,
        class: u16,
        mode: u8,
        spread: i32,
        flags: u32,
    ) -> Option<UnitId> {
        if self.drlg_room(room) {
            return self.spawn_unit(room, x, y, class, mode);
        }
        self.inner
            .spawn_monster_flags(room, x, y, class, mode, spread, flags)
    }
    /// `0x0056D130`: a portal object of `class` in mode 1 to `level`,
    /// owned by `owner`, at (x, y) or the free spot next to it
    /// (PROVISIONAL, REC-127: the body is unwritten; the town portal's
    /// owner and level fields are reused).
    #[allow(clippy::too_many_arguments)]
    fn open_portal(
        &mut self,
        owner: Option<UnitId>,
        room: RoomId,
        x: i32,
        y: i32,
        level: u32,
        class: u16,
        exact: bool,
    ) -> Option<UnitId> {
        if self.inner.econ.hooks.objects.is_none() || !self.drlg_room(room) {
            return self
                .inner
                .open_portal(owner, room, x, y, level, class, exact);
        }
        let (room, x, y) = if exact {
            (room, x, y)
        } else {
            helpers::free_spot(self, room, x, y, 2, 0x3E01, 100)
                .map_or((room, x, y), |(fx, fy, fr)| (fr, fx, fy))
        };
        let portal = self.view(|g, v| v.create_object(g, room, u32::from(class), x, y, 1))?;
        let guid = owner.map(|o| self.inner.econ.game.lists.unit(o).map_or(0, |u| u.guid));
        if let Some(d) = self
            .inner
            .econ
            .hooks
            .objects
            .as_mut()
            .and_then(|st| st.control.data.get_mut(&portal))
        {
            d.interact = u8::try_from(level).unwrap_or(u8::MAX);
            d.owner = guid.map(|g| g as i32);
        }
        Some(portal)
    }
    /// `0x0056EDE0` ([`helpers::missile_at_point`]) on the missile
    /// store when the action wiring holds one; else the rest's.
    fn create_missile(
        &mut self,
        owner: UnitId,
        skill: u16,
        level: u8,
        class: u16,
        x: i32,
        y: i32,
    ) -> Option<UnitId> {
        if self.inner.econ.hooks.missiles.is_some() {
            return helpers::missile_at_point(self, owner, skill, level, class, x, y);
        }
        self.inner.create_missile(owner, skill, level, class, x, y)
    }
    /// `0x0064A710` / `0x0064A760` on a missile of the store.
    fn set_missile_target(&mut self, missile: UnitId, a: u32, b: u32) {
        if let Some(d) = self
            .inner
            .econ
            .hooks
            .missiles
            .as_mut()
            .and_then(|s| s.get_mut(missile))
        {
            d.target = (a as i32, b as i32);
            return;
        }
        self.inner.set_missile_target(missile, a, b)
    }
    /// `0x0061AED0(room, 0)` on the unit's room
    /// ([`crate::wiring::action::DrlgWorld::refresh_room`]); a unit of
    /// the lists without a room: nothing (`0x0061AED6`).
    fn refresh_room(&mut self, unit: UnitId) {
        let e = &mut *self.inner.econ;
        if let Some(u) = e.game.lists.unit(unit) {
            let Some(room) = u.room() else { return };
            if e.hooks.drlg.refresh_room(e.game, room, false) {
                return;
            }
        }
        self.inner.refresh_room(unit)
    }
    fn client_save_flags(&self, player: UnitId) -> Option<u16> {
        self.inner.client_save_flags(player)
    }
    fn set_client_save_flags(&mut self, player: UnitId, flags: u16) {
        self.inner.set_client_save_flags(player, flags)
    }
    fn unhandled(&mut self, chain: u8, function: u32) {
        self.inner.unhandled(chain, function)
    }
    fn client_in_act(&mut self, player: UnitId, act: u8) -> bool {
        self.inner.client_in_act(player, act)
    }
    fn start_tainted_sun(&mut self, act: u8) {
        self.inner.start_tainted_sun(act)
    }
    fn end_tainted_sun(&mut self) {
        self.inner.end_tainted_sun()
    }
    fn quest_chest_gate(&mut self, object: UnitId, player: UnitId) -> bool {
        self.inner.quest_chest_gate(object, player)
    }
    fn quest_drop(
        &mut self,
        unit: UnitId,
        code: [u8; 4],
        quality: u8,
        level: Option<i32>,
        droppable: bool,
    ) -> Option<UnitId> {
        // `level` is the out-parameter `0x00559A30` overwrites before it
        // reads it (§9 rule 2, `quests-act3-2.md` §11.3): not an input here.
        match self.econ_quest_drop(unit, code, quality, i32::from(droppable)) {
            Some(item) => item,
            None => self.inner.quest_drop(unit, code, quality, level, droppable),
        }
    }
    /// `0x00585B90(op, kind)` on an object with object data when the
    /// game holds the object drop state (`ActionHooks::object_drops`,
    /// [`super::object_chest_drop`] with the object tables' `levels`).
    fn object_treasure(&mut self, object: UnitId, operator: UnitId, kind: u8) {
        let known = self.known(object);
        let e = &mut *self.inner.econ;
        let tables = e.hooks.objects.as_ref().map(|s| s.tables.clone());
        let (true, Some(t), true) = (known, tables, e.hooks.object_drops.is_some()) else {
            return self.inner.object_treasure(object, operator, kind);
        };
        let Some(mut d) = e.hooks.object_drops.take() else {
            return;
        };
        // The economy holds the game's item store, game seed and unique
        // bits for this call: hand them back to their home in the hooks
        // for the drop, and take them again after it.
        std::mem::swap(&mut e.hooks.items, &mut *e.items);
        e.hooks.game_seed = e.fields.seed;
        e.hooks.uniques = std::mem::take(&mut e.fields.uniques);
        let mut sim = crate::units::hooks::Sim {
            game: &mut *e.game,
            units: &mut *e.units,
            stats: &mut *e.stats,
            data: e.data,
        };
        super::object_chest_drop(
            &mut *e.hooks,
            &mut sim,
            &mut d,
            &t.levels,
            &mut super::NoSpot,
            object,
            Some(operator),
            kind,
        );
        std::mem::swap(&mut e.hooks.items, &mut *e.items);
        e.fields.seed = e.hooks.game_seed;
        e.fields.uniques = std::mem::take(&mut e.hooks.uniques);
        e.hooks.object_drops = Some(d);
    }
    /// `0x00585970(game, object, 'gld ', 2)`
    /// ([`super::drop_helpers::code_drop`], PROVISIONAL there).
    fn drop_gold(&mut self, object: UnitId) {
        let gold = u32::from_le_bytes(*b"gld ");
        if self
            .with_drop_state(|h, sim, d, levels, spots| {
                super::drop_helpers::code_drop(h, sim, d, levels, spots, object, gold, 2)
            })
            .is_none()
        {
            self.inner.drop_gold(object)
        }
    }
    fn set_room_portal(&mut self, room: RoomId, on: bool) {
        self.inner.set_room_portal(room, on)
    }
    fn spawn_quest_object(&mut self, room: RoomId, x: i32, y: i32, class: u16) -> Option<UnitId> {
        self.inner.spawn_quest_object(room, x, y, class)
    }
    fn player_busy(&mut self, player: UnitId) -> bool {
        self.inner.player_busy(player)
    }
    fn open_insert_dialog(&mut self, player: UnitId, object: UnitId) {
        self.inner.open_insert_dialog(player, object)
    }
    /// `missiles.txt` `Range` (u16 +0x96) of the action tables' row
    /// (`quests-act2.md` §8.7); a row outside the table is `None`.
    fn missile_range(&mut self, row: u32) -> Option<i32> {
        let t = &self.inner.econ.hooks.tables.missiles;
        Some(i32::from(t.get(row as usize)?.range))
    }
    fn is_trading(&mut self, player: UnitId) -> bool {
        self.inner.is_trading(player)
    }
    fn remove_unit(&mut self, unit: UnitId) {
        self.inner.remove_unit(unit)
    }
    fn npc_hold_chat(&mut self, npc: UnitId) -> bool {
        self.inner.npc_hold_chat(npc)
    }
    fn spawn_location(&mut self, act: u8, level: u32, kind: u8) -> Option<(i32, i32, RoomId)> {
        self.inner.spawn_location(act, level, kind)
    }
    fn free_spot_near(
        &mut self,
        room: RoomId,
        x: i32,
        y: i32,
        size: u32,
        mask: u32,
        radius: u32,
    ) -> Option<(i32, i32, RoomId)> {
        self.inner.free_spot_near(room, x, y, size, mask, radius)
    }
    fn npc_intro_heard(&mut self, player: UnitId, class: u16) -> bool {
        self.inner.npc_intro_heard(player, class)
    }
    fn set_npc_intro(&mut self, player: UnitId, class: u16) {
        self.inner.set_npc_intro(player, class)
    }

    // -- The helpers' narrow seams (`quests-helpers.md`) on the action
    // wiring.

    /// `0x00619730`: the DRLG room's sub-tile box.
    fn room_box(&mut self, room: RoomId) -> Option<TileRect> {
        let e = &*self.inner.econ;
        match e.hooks.drlg.subtiles(e.game, room) {
            Some(t) => Some(t),
            None => self.inner.room_box(room),
        }
    }
    /// `0x0064D800(room, x, y, size, size, mask)` on the DRLG collision
    /// (`sim/path-placement.md` §4, [`Rooms`]).
    fn box_collides(&mut self, room: RoomId, x: i32, y: i32, size: i32, mask: u32) -> bool {
        if self.drlg_room(room) {
            let e = &*self.inner.econ;
            let n = size as u32;
            return Rooms(&e.hooks.drlg).box_query(room, x, y, n, n, mask) != 0;
        }
        self.inner.box_collides(room, x, y, size, mask)
    }
    /// `0x0059FA30` (`missiles/missiles.md` §R2.3) on the action wiring's
    /// missile store; no store: the rest's answer.
    fn spawn_missile(&mut self, rec: QuestMissile) -> Option<UnitId> {
        let e = &mut *self.inner.econ;
        let Some(mut store) = e.hooks.missiles.take() else {
            return self.inner.spawn_missile(rec);
        };
        let p = MissileParams {
            flags: rec.flags,
            owner: Some(rec.owner),
            origin: rec.origin,
            class: i32::from(rec.class),
            x: rec.x,
            y: rec.y,
            target_x: rec.target_x,
            target_y: rec.target_y,
            skill: i32::from(rec.skill),
            level: i32::from(rec.level),
            ..MissileParams::default()
        };
        let t = e.hooks.tables.clone();
        let made = {
            let mut v = View::of(&mut *e.units, &mut *e.stats, e.data, &mut *e.hooks);
            let mut cx = missiles::Ctx {
                tables: &t.missiles,
                store: &mut store,
                world: &mut v,
            };
            missiles::create_missile(&mut *e.game, &mut cx, &p)
        };
        e.hooks.missiles = Some(store);
        made
    }
    /// `0x0064A710`: data +0x28 of a missile of the store.
    fn set_missile_guid(&mut self, missile: UnitId, v: u32) {
        if let Some(d) = self
            .inner
            .econ
            .hooks
            .missiles
            .as_mut()
            .and_then(|s| s.get_mut(missile))
        {
            d.target.0 = v as i32;
            return;
        }
        self.inner.set_missile_guid(missile, v)
    }
    /// `0x00552F60(game, kind, guid)` on the game's unit lists.
    fn unit_by_guid(&mut self, kind: u8, guid: u32) -> Option<UnitId> {
        let ty = UnitType::ALL.into_iter().find(|&t| t as u8 == kind)?;
        self.inner.econ.game.lists.find_unit(ty, guid)
    }
    /// `objects.txt` `Mode1` of an object with object data.
    fn object_mode1(&mut self, object: UnitId) -> Option<bool> {
        if let Some(st) = self.inner.econ.hooks.objects.as_ref() {
            if let Some(d) = st.control.data.get(&object) {
                if let Ok(o) = st.tables.object(d.class) {
                    return Some(o.mode1 != 0);
                }
            }
        }
        self.inner.object_mode1(object)
    }
    /// Unit +0xC4 &= !`flags` on a unit record.
    fn clear_unit_flags(&mut self, unit: UnitId, flags: u32) {
        match self.inner.econ.units.get_mut(unit) {
            Some(r) => r.flags &= !flags,
            None => self.inner.clear_unit_flags(unit, flags),
        }
    }
    /// The type-5 units of the object's room unit list, in list order.
    fn room_warp_tiles(&mut self, object: UnitId) -> Vec<UnitId> {
        let lists = &self.inner.econ.game.lists;
        let Some(room) = lists.unit(object).and_then(|u| u.room()) else {
            return Vec::new();
        };
        lists
            .room_units(room)
            .into_iter()
            .filter(|&u| lists.unit(u).is_some_and(|e| e.ty == UnitType::Tile))
            .collect()
    }
    /// `0x005550B0(game, player, tile)` (`sim/path-placement.md` §12.2,
    /// [`crate::wiring::path::place::warp_player`]).
    fn warp_through(&mut self, player: UnitId, tile: UnitId) {
        let e = &*self.inner.econ;
        let room = e.game.lists.unit(tile).and_then(|u| u.room());
        let class = e.units.get(tile).map(|r| r.class);
        let (Some(room), Some(class)) = (room, class) else {
            return;
        };
        self.view(|g, v| {
            crate::wiring::path::place::warp_player(
                crate::wiring::path::PathCtx::of(v, g),
                player,
                room,
                class,
            )
        });
    }

    // -- The quest helpers' host seams (`quests-helpers.md` §4.2, §5, §7;
    // `quests-act3.md` §6) on the action wiring and the lent host parts.

    /// `0x00558200(player, 0)` (`quests-act5.md` open question 2,
    /// [`quest_reward::item_level`]) on the unit's record and stats.
    fn quest_item_level(&mut self, player: UnitId) -> i32 {
        match quest_reward::item_level(&*self.inner.econ, player) {
            Some(v) => v,
            None => self.inner.quest_item_level(player),
        }
    }
    /// `0x0056D2C0` (`skills/bodies.md` §2.4) on the path provider: the
    /// path target unit's position, else the target point; `None` when
    /// a coordinate is 0. No path provider: the rest's.
    fn path_target_xy(&mut self, unit: UnitId) -> Option<(i32, i32)> {
        match self.view(|g, v| v.path_target_position(g, unit)) {
            Some(p) => p,
            None => self.inner.path_target_xy(unit),
        }
    }
    /// `0x00568060(game, player, button, 0)` (`world/vendors-2.md` §10.1):
    /// no player → nothing; no active interaction → S→C 0x77 0x0C;
    /// buttons 0x12–0x14, 0x17, 0x18 (stash, cube) → their owners (the
    /// rest); an interaction other than a player → 0x77 0x0D; a trade
    /// partner gone → 0x77 0x0C; with the partner, buttons 5 and 6 and
    /// those outside 2–8 do nothing, the other trade buttons are the
    /// player-trade flow's (no spec, §10.3: the rest).
    ///
    /// PROVISIONAL (world/vendors-2.md §10.1 rule 5; REC-none): with the
    /// partner gone `0x00597A20(game, P)` is read as 0 (its body is not
    /// written), so 0x77 0x0C is sent.
    fn trade_button(&mut self, player: UnitId, button: u8) {
        if self.inner.econ.units.get(player).is_none() {
            return;
        }
        let Some((ty, guid)) = self.interact_unit(player) else {
            return self.send(player, &trade_action(0x0C));
        };
        match button {
            0x12..=0x14 | 0x17 | 0x18 => self.inner.trade_button(player, button),
            _ if ty != 0 => self.send(player, &trade_action(0x0D)),
            _ if self.player_by_guid(guid).is_none() => {
                self.send(player, &trade_action(0x0C));
            }
            2..=4 | 7 | 8 => self.inner.trade_button(player, button),
            _ => {}
        }
    }
    /// `0x00572E00` on the NPC's interaction list (lent with
    /// [`HostQuests::chats`]): the player's node unlinked; nothing for a
    /// unit without a list (not an `interact` monster).
    fn free_chat_node(&mut self, npc: UnitId, player: UnitId) {
        let Some(chats) = self.chats.as_deref_mut() else {
            return self.inner.free_chat_node(npc, player);
        };
        if let Some(l) = chats.get_mut(&npc) {
            if let Some(i) = l.nodes.iter().position(|n| n.0 == player) {
                l.nodes.remove(i);
            }
        }
    }
    /// `0x00573180`'s end (`quests-act5.md` §5.7) on the NPC's interaction
    /// list: every node freed, the list emptied.
    fn clear_npc_chats(&mut self, npc: UnitId) {
        let Some(chats) = self.chats.as_deref_mut() else {
            return self.inner.clear_npc_chats(npc);
        };
        if let Some(l) = chats.get_mut(&npc) {
            l.nodes.clear();
        }
    }
    /// `0x005852E0(game, player GUID, object GUID, 0, 2)`: the C→S 0x44
    /// cancel (`quests-act2-2.md` §3.2 step 3, [`act2::q6::insert_cancel`]).
    fn obelisk_close(&mut self, player: UnitId, object: UnitId) {
        let g = self.guid(object);
        act2::q6::insert_cancel(self, player, object, g);
    }
    /// `0x00567330` → `0x0055FA40` (`items/inventory.md` §5.5) on the lent
    /// inventory model; none lent: the rest's.
    fn close_cube(&mut self, player: UnitId) {
        match self.inventory.as_deref_mut() {
            Some(inv) => inv.inventory_pass(&mut *self.inner.econ, player),
            None => self.inner.close_cube(player),
        }
    }
    /// `0x005353F0`: player data +0x48 through the action wiring's one
    /// seam for it (`Pending::object_portal_guid`, the portal operate's,
    /// `objects.md` §12); a unit other than a player has no player data.
    fn town_portal_guid(&mut self, player: UnitId) -> Option<u32> {
        match self.inner.econ.units.get(player).map(|r| r.ty) {
            Some(UnitType::Player) => Some(self.inner.econ.hooks.x.object_portal_guid(player)),
            Some(_) => None,
            None => self.inner.town_portal_guid(player),
        }
    }
    /// `0x00553720` through the action wiring's seam the portal operate
    /// uses (`Pending::object_portal_partner`, `objects.md` §12 rule 6).
    fn portal_partner(&mut self, portal: UnitId) -> Option<UnitId> {
        self.view(|g, v| v.h.x.object_portal_partner(g, portal))
    }
    /// `quests-helpers.md` §7 step 4 = `objects.md` §12 rule 12's removal
    /// (`0x0061A270`, `0x00555600`, `0x0061AED0(room, 1)`) through the
    /// action wiring's seam for it (`Pending::object_remove_portal`).
    fn free_portal_object(&mut self, portal: UnitId) {
        self.view(|g, v| v.h.x.object_remove_portal(g, portal));
    }
    /// `0x005DDFC0(game, monster, mode, x, y)` (`monsters/ai.md` §7.1: the
    /// mode request at a point, no path step set) on the action wiring's
    /// monster mode change; a unit that is not a monster: the rest's.
    fn monster_mode_at(&mut self, monster: UnitId, mode: u8, x: i32, y: i32) {
        let is_monster = self
            .inner
            .econ
            .units
            .get(monster)
            .is_some_and(|r| r.ty == UnitType::Monster);
        if !is_monster {
            return self.inner.monster_mode_at(monster, mode, x, y);
        }
        self.view(|g, v| {
            AiModes::change_mode(v, g, monster, mode, ModeTarget::Point(x, y));
        });
    }
}
