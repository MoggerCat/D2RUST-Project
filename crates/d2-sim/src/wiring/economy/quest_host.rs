// Spec: specs/world/quests.md §9; specs/world/quests-act1.md §10; specs/world/quests-act1-rest.md §1–§3; specs/world/quests-act2.md §1.5; specs/world/objects.md §3, §4, §7; specs/sim/tick.md §5.2
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
//!   object code's footprint seam, `Pending::object_free_footprint`);
//! - the level of a unit in a DRLG room (`DrlgWorld::level_id`);
//! - `0x006280D0(item, 0x10)` on an item of the game's item store;
//! - `missiles.txt` `Range` from the action tables;
//! - the chest treasure `0x00585B90(op, kind)` on the object drop state
//!   (`ActionHooks::object_drops`, [`super::object_chest_drop`]).
//!
//! An object without object data, a unit outside a DRLG room and an item
//! outside the store keep the rest's answer, as before.
//!
//! Status: wired, unverified.

use crate::rng::Seed;
use crate::units::{RoomId, UnitId};
use crate::wiring::action::{ActionHooks, Pending, View};
use crate::world::quests::{PlayerQuests, QuestChain, QuestWorld, UnitKind};

use super::{EconomyQuests, QuestRest};

/// [`EconomyQuests`] on the action wiring's hooks, with the calls the
/// action wiring provides answered there (module doc).
pub struct HostQuests<'e, 'a, X, R> {
    pub inner: EconomyQuests<'e, 'a, ActionHooks<X>, R>,
}

impl<'e, 'a, X: Pending, R: QuestRest> HostQuests<'e, 'a, X, R> {
    pub fn new(inner: EconomyQuests<'e, 'a, ActionHooks<X>, R>) -> Self {
        Self { inner }
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

    /// Runs `f` on the action wiring's view over the economy's parts.
    fn view<T>(&mut self, f: impl FnOnce(&mut crate::game::Game, &mut View<'_, X>) -> T) -> T {
        let e = &mut *self.inner.econ;
        let mut v = View::of(&mut *e.units, &mut *e.stats, e.data, &mut *e.hooks);
        f(&mut *e.game, &mut v)
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
    /// (`Pending::object_free_footprint`, `objects.md` §8.2, §10).
    fn free_object_collision(&mut self, object: UnitId) {
        if self.known(object) {
            let e = &mut *self.inner.econ;
            return e.hooks.x.object_free_footprint(e.game, object);
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
        self.inner
            .reward_item(player, code, level, quality, droppable)
    }
    fn drop_item_at(&mut self, unit: UnitId, code: [u8; 4], quality: u8) -> bool {
        self.inner.drop_item_at(unit, code, quality)
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
    fn free_spot(
        &mut self,
        player: UnitId,
        size: u32,
        mask: u32,
        radius: u32,
        limit: u32,
    ) -> Option<(i32, i32)> {
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
        self.inner
            .free_spot_at(room, x, y, size, mask, radius, limit)
    }
    fn spawn_monster(
        &mut self,
        room: RoomId,
        x: i32,
        y: i32,
        class: u16,
        mode: u8,
        r: u32,
    ) -> Option<UnitId> {
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
    fn remove_monster(&mut self, monster: UnitId) {
        self.inner.remove_monster(monster)
    }
    fn drop_preset_monster(&mut self, act: u8, class: u16) {
        self.inner.drop_preset_monster(act, class)
    }
    fn find_object_near(&self, object: UnitId, class: u16) -> Option<UnitId> {
        self.inner.find_object_near(object, class)
    }
    fn create_object(&mut self, room: RoomId, x: i32, y: i32, class: u16) -> Option<UnitId> {
        self.inner.create_object(room, x, y, class)
    }
    fn open_quest_message(&mut self, player: UnitId, object: UnitId, msg: u16) {
        self.inner.open_quest_message(player, object, msg)
    }
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
        self.inner
            .spawn_monster_flags(room, x, y, class, mode, spread, flags)
    }
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
        self.inner
            .open_portal(owner, room, x, y, level, class, exact)
    }
    fn create_missile(
        &mut self,
        owner: UnitId,
        skill: u16,
        level: u8,
        class: u16,
        x: i32,
        y: i32,
    ) -> Option<UnitId> {
        self.inner.create_missile(owner, skill, level, class, x, y)
    }
    fn set_missile_target(&mut self, missile: UnitId, a: u32, b: u32) {
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
        self.inner.quest_drop(unit, code, quality, level, droppable)
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
    fn drop_gold(&mut self, object: UnitId) {
        self.inner.drop_gold(object)
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
    fn unit_distance(&mut self, a: UnitId, b: UnitId) -> i32 {
        self.inner.unit_distance(a, b)
    }
    fn living_player_within(&mut self, unit: UnitId, radius: i32) -> bool {
        self.inner.living_player_within(unit, radius)
    }
    fn npc_intro_heard(&mut self, player: UnitId, class: u16) -> bool {
        self.inner.npc_intro_heard(player, class)
    }
    fn set_npc_intro(&mut self, player: UnitId, class: u16) {
        self.inner.set_npc_intro(player, class)
    }
}
