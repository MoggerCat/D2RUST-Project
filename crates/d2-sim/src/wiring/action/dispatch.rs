// Spec: specs/sim/tick.md §3, §5.5, §5.6; specs/sim/intents-events.md §7.3, §7.5, §8.3; specs/audio/triggers-2.md §14; specs/world/objects.md §2, §14; specs/sim/units.md §5; specs/drlg/rooms.md §4.1, §7, §8; specs/drlg/levels.md §9
//! [`ActionSim`]: the one dispatcher the tick runs. Timer events go to
//! the unit dispatch (`units.md` §5: the per-kind handler tables and the
//! monster freeze drop of `tick.md` §5.6), whose hooks run the missile
//! class handler, the AI think and the AI reset ([`super::units`]). The
//! tick hooks of steps 9 and 10 and the client room change run the DRLG
//! ([`super::rooms`]); every other tick hook keeps its default.

use std::sync::Arc;

use crate::game::Game;
use crate::missiles;
use crate::monsters::ai;
use crate::stats::StatData;
use crate::tick::timer::TimerRun;
use crate::tick::{EventDispatch, TickHooks};
use crate::units::dispatch::UnitSystem;
use crate::units::hooks::UnitData;
use crate::units::lists::client_state;
use crate::units::{ClientId, RoomId, UnitId, UnitType};
use crate::world::objects::{ObjectControl, ObjectTables};

use super::combat::CombatView;
use super::objects::{ObjectCase, ObjectState, ObjectView};
use super::vitals_sync;
use super::waypoints::WaypointView;
use super::{ActionHooks, ActionTables, Pending, View, WiringError};

/// The action systems of one game, as one [`EventDispatch`] and
/// [`TickHooks`] for [`crate::tick::tick`].
pub struct ActionSim<X> {
    pub sys: UnitSystem<ActionHooks<X>>,
}

impl<X: Pending> ActionSim<X> {
    /// Sends `m` to the player `p` after the host's item update pass when
    /// the player has item messages pending (+0xC8 bit 0) and the host
    /// defers ([`ActionHooks::defer_player_tail`]): in 1.14d the item
    /// messages are part of the player's step 5 update, before the rest of
    /// the client update (recorded `items-drops-rbo-00` frame 50).
    fn send_after_items(&mut self, p: UnitId, m: &[u8]) {
        let h = &mut self.sys.hooks;
        let pending = self.sys.units.get(p).is_some_and(|r| r.flags2 & 1 != 0);
        if h.defer_player_tail && pending {
            h.player_tail.push((p, m.to_vec()));
        } else {
            h.x.send(p, m);
        }
    }

    /// Sends the player step 5 / 7 messages held by
    /// [`ActionHooks::defer_player_tail`]: the host calls it after its item
    /// update pass.
    pub fn flush_player_tail(&mut self) {
        for (receiver, m) in std::mem::take(&mut self.sys.hooks.player_tail) {
            self.sys.hooks.x.send(receiver, &m);
        }
    }

    pub fn new(stat_data: Arc<StatData>, data: UnitData, hooks: ActionHooks<X>) -> Self {
        Self {
            sys: UnitSystem::new(stat_data, data, hooks),
        }
    }

    /// The shared state.
    pub fn hooks(&mut self) -> &mut ActionHooks<X> {
        &mut self.sys.hooks
    }

    /// Runs `f` on a [`View`] of the unit side (allocation, removal,
    /// stats, states) with the game.
    pub fn with<R>(
        &mut self,
        game: &mut Game,
        f: impl FnOnce(&mut Game, &mut View<'_, X>) -> R,
    ) -> R {
        let s = &mut self.sys;
        let mut v = View::of(&mut s.units, &mut s.stats, &s.data, &mut s.hooks);
        f(game, &mut v)
    }

    /// The join's Iron Golem re-summon for `player`
    /// ([`Pending::golem_resummon`]).
    pub fn golem_resummon(&mut self, game: &mut Game, player: UnitId) -> bool {
        let s = &mut self.sys;
        let mut sim = crate::units::hooks::Sim {
            game,
            units: &mut s.units,
            stats: &mut s.stats,
            data: &s.data,
        };
        X::golem_resummon(&mut s.hooks, &mut sim, player)
    }

    /// The save load's right-skill aura start of `player`
    /// ([`Pending::right_aura_select`]).
    pub fn right_aura_select(&mut self, game: &mut Game, player: UnitId) {
        let s = &mut self.sys;
        let mut sim = crate::units::hooks::Sim {
            game,
            units: &mut s.units,
            stats: &mut s.stats,
            data: &s.data,
        };
        X::right_aura_select(&mut s.hooks, &mut sim, player)
    }

    /// The save load's passive states of `unit`
    /// ([`Pending::passive_refresh_all`]).
    pub fn passive_refresh_all(&mut self, game: &mut Game, unit: UnitId) {
        let s = &mut self.sys;
        let mut sim = crate::units::hooks::Sim {
            game,
            units: &mut s.units,
            stats: &mut s.stats,
            data: &s.data,
        };
        X::passive_refresh_all(&mut s.hooks, &mut sim, unit)
    }

    /// The passive states' unit bits at the end of the join
    /// ([`Pending::passive_states_on`]).
    pub fn passive_states_on(&mut self, game: &mut Game, unit: UnitId) {
        let s = &mut self.sys;
        let mut sim = crate::units::hooks::Sim {
            game,
            units: &mut s.units,
            stats: &mut s.stats,
            data: &s.data,
        };
        X::passive_states_on(&mut s.hooks, &mut sim, unit)
    }

    /// The pet follow of the summoned pet types
    /// ([`Pending::summon_follow`]).
    pub fn summon_follow(&mut self, game: &mut Game, player: UnitId) {
        let s = &mut self.sys;
        let mut sim = crate::units::hooks::Sim {
            game,
            units: &mut s.units,
            stats: &mut s.stats,
            data: &s.data,
        };
        X::summon_follow(&mut s.hooks, &mut sim, player)
    }

    /// The aura of the loaded right skill ([`Pending::assign_right_aura`]),
    /// after the load selected the hands.
    pub fn assign_right_aura(&mut self, game: &mut Game, unit: UnitId) {
        let s = &mut self.sys;
        let mut sim = crate::units::hooks::Sim {
            game,
            units: &mut s.units,
            stats: &mut s.stats,
            data: &s.data,
        };
        X::assign_right_aura(&mut s.hooks, &mut sim, unit)
    }

    /// Runs `f` with the missile code's context (creation from skills,
    /// tests).
    pub fn missiles<R>(
        &mut self,
        game: &mut Game,
        f: impl FnOnce(&mut Game, &mut missiles::Ctx<'_, View<'_, X>>) -> R,
    ) -> Option<R> {
        let s = &mut self.sys;
        let mut store = s.hooks.missiles.take()?;
        let t = s.hooks.tables.clone();
        let r = {
            let mut v = View::of(&mut s.units, &mut s.stats, &s.data, &mut s.hooks);
            let mut cx = missiles::Ctx {
                tables: &t.missiles,
                store: &mut store,
                world: &mut v,
            };
            f(game, &mut cx)
        };
        s.hooks.missiles = Some(store);
        Some(r)
    }

    /// Runs `f` with the AI code's context (AI install at monster
    /// creation, `monsters/init.md`; tests).
    pub fn ai<R>(
        &mut self,
        game: &mut Game,
        f: impl FnOnce(&mut Game, &mut ai::Ctx<'_, View<'_, X>>) -> R,
    ) -> Option<R> {
        let s = &mut self.sys;
        let mut store = s.hooks.ai.take()?;
        let t = s.hooks.tables.clone();
        let info = s.hooks.ai_info;
        let r = {
            let mut v = View::of(&mut s.units, &mut s.stats, &s.data, &mut s.hooks);
            let mut cx = ai::Ctx {
                tables: ai::AiTables {
                    monstats: &t.combat.monstats,
                    monstats2: &t.combat.monstats2,
                    levels: &t.levels,
                    skill_modes: &t.skill_modes,
                    skills: &t.skills.skills,
                    missiles: &t.skills.missiles,
                },
                info,
                store: &mut store,
                world: &mut v,
            };
            f(game, &mut cx)
        };
        s.hooks.ai = Some(store);
        Some(r)
    }

    /// Runs `f` with combat's view and tables (skill functions, tests).
    pub fn combat<R>(
        &mut self,
        game: &mut Game,
        f: impl FnOnce(&mut CombatView<'_, X>, &ActionTables) -> R,
    ) -> R {
        let s = &mut self.sys;
        let t = s.hooks.tables.clone();
        let mut w = CombatView {
            game,
            v: View::of(&mut s.units, &mut s.stats, &s.data, &mut s.hooks),
        };
        f(&mut w, &t)
    }

    /// Runs `f` with the waypoint code's view (object init/operate, the
    /// C→S 0x49 handler).
    pub fn waypoints<R>(
        &mut self,
        game: &mut Game,
        f: impl FnOnce(&mut WaypointView<'_, X>) -> R,
    ) -> R {
        let s = &mut self.sys;
        let mut w = WaypointView {
            game,
            v: View::of(&mut s.units, &mut s.stats, &s.data, &mut s.hooks),
        };
        f(&mut w)
    }

    /// Game creation's object control `0x00546C60` (`objects.md` §2): one
    /// step of the game seed ([`ActionHooks::game_seed`]), the control,
    /// the shrine lists and the level regions from `tables`.
    ///
    /// Game creation derives, in order, the monster regions, this
    /// control, the NPC control and the quest control, each from one
    /// game-seed step (`rng.md` §5.2); `WorldSim::create_game` runs that
    /// sequence.
    pub fn create_objects(&mut self, tables: Arc<ObjectTables>) {
        let h = &mut self.sys.hooks;
        h.objects = Some(ObjectState::new(&mut h.game_seed, tables));
    }

    /// A host holding the quest control takes the object module's quest
    /// routes from a queue ([`ActionSim::take_quest_calls`]) instead of
    /// [`super::Pending::object_route`]; call right after
    /// [`ActionSim::create_objects`]. No object state: nothing.
    pub fn route_quest_objects(&mut self) {
        if let Some(st) = self.sys.hooks.objects.as_mut() {
            st.route_quests();
        }
    }

    /// The queued quest routes ([`super::QuestObjectCall`]), in order.
    pub fn take_quest_calls(&mut self) -> Vec<super::QuestObjectCall> {
        self.sys
            .hooks
            .objects
            .as_mut()
            .map(|s| s.take_quest_calls())
            .unwrap_or_default()
    }

    /// Runs `f` with the object control, the tables and the object
    /// code's view (tests, skills that call the dispatch directly).
    /// `None`: no object state.
    pub fn objects<R>(
        &mut self,
        game: &mut Game,
        f: impl FnOnce(&mut ObjectControl, &ObjectTables, &mut ObjectView<'_, X>) -> R,
    ) -> Option<R> {
        let s = &mut self.sys;
        let mut v = View::of(&mut s.units, &mut s.stats, &s.data, &mut s.hooks);
        super::objects::with_objects(game, &mut v, f)
    }

    /// The C→S 0x13 object case (`waypoints.md` §5.2,
    /// [`View::object_message`]). `None`: no object state.
    pub fn operate_object_message(
        &mut self,
        game: &mut Game,
        player: UnitId,
        guid: u32,
    ) -> Option<ObjectCase> {
        self.with(game, |g, v| v.object_message(g, player, guid))
    }

    /// The C→S 0x13 tile case ([`View::warp_tile_message`]). `None`: no
    /// path provider.
    pub fn warp_tile_message(&mut self, game: &mut Game, player: UnitId, guid: u32) -> Option<u32> {
        self.with(game, |g, v| v.warp_tile_message(g, player, guid))
    }

    /// The Town Portal cast of `player` without an item
    /// ([`View::town_portal_cast`], `objects-2.md` §27.1): the pair's
    /// units (object 1 next to the player, object 2 in town). `None`:
    /// refused or not made.
    pub fn open_town_portal(
        &mut self,
        game: &mut Game,
        player: UnitId,
    ) -> Option<(UnitId, UnitId)> {
        self.with(game, |g, v| {
            let (made, _) = v.town_portal_cast(g, player);
            if made == 0 {
                return None;
            }
            let g1 = v.h.portals.player_portal(player)?;
            let o1 = g.lists.find_unit(crate::units::UnitType::Object, g1)?;
            Some((o1, v.portal_partner(g, o1)?))
        })
    }

    fn log(&mut self, r: Result<(), WiringError>) {
        if let Err(e) = r {
            self.sys.hooks.errors.push(e);
        }
    }
}

impl<X: Pending> EventDispatch for ActionSim<X> {
    fn run_event(&mut self, game: &mut Game, run: &TimerRun) {
        self.sys.run_event(game, run);
    }
}

/// Flag-ex (+0xC8) bit 21: the per-client update's inventory refresh
/// (`tick.md` §6 rule 5; set by the join's item messages,
/// `intents-events.md` §8.2 rule 3.5).
pub const INVENTORY_REFRESH_EX: u32 = 0x0020_0000;

impl<X: Pending> ActionSim<X> {
    /// The freed ground items' removal records (PROVISIONAL, REC-281).
    fn send_removed_items(&mut self, game: &mut Game, client: ClientId) {
        let h = &mut self.sys.hooks;
        if h.removed_items.is_empty() {
            return;
        }
        let Some(c) = game.lists.client(client) else {
            return;
        };
        let Some(player) = c.player else {
            return;
        };
        let adjacent = c
            .room
            .and_then(|r| game.lists.room(r))
            .map(|r| r.adjacent.clone())
            .unwrap_or_default();
        for &(guid, room) in &h.removed_items {
            if adjacent.contains(&room) {
                let m = crate::units::messages::remove_unit(UnitType::Item as u8, guid);
                h.x.send(player, &m);
            }
        }
    }
}

impl<X: Pending> TickHooks for ActionSim<X> {
    /// Per-client update removals (`0x0053A770`, `tick.md` §6 rule 5):
    /// S→C 0x0A (`messages::remove_unit`) to the client's player for each
    /// removal record in the client room's adjacent rooms. PROVISIONAL
    /// (REC-281): the records are the freed ground items'
    /// (`ActionHooks::removed_items`).
    fn send_removed_units(&mut self, game: &mut Game, client: ClientId) {
        self.send_removed_items(game, client);
        // The room delete lists (`tick.md` §6.5, [`View::send_room_deletes`]).
        self.with(game, |g, v| v.send_room_deletes(g, client));
    }

    /// Step 7 (`0x0061A2C0`): the room's removal records are freed.
    fn free_removal_records(&mut self, _: &mut Game, room: RoomId) {
        self.sys.hooks.removed_items.retain(|&(_, r)| r != room);
        self.sys.hooks.room_deletes.remove(&room);
    }

    /// Step 1 `0x0061C040(act, a)` (`render/lighting.md` §9.3 rule 5):
    /// the act's environment record advanced with `A` = the act index.
    /// An act no join has built yet ([`crate::units::lists::ActEntry::built`])
    /// does not exist in 1.14d and is skipped.
    fn advance_environment(&mut self, game: &mut Game, act: u8) -> bool {
        game.lists
            .act_mut(act)
            .filter(|a| a.built)
            .is_some_and(|a| a.environment.server_advance(act))
    }

    /// Step 1 per client (`tick.md` §3): the player's items refreshed
    /// (`0x0055FDE0`, [`Pending::environment_refresh_items`]), then the
    /// 0x53 with the record's values (`0x0061C330(act)`) when the client
    /// is in game (state 4) in that act (its room's act).
    fn environment_changed(&mut self, game: &mut Game, act: u8, client: ClientId) {
        let Some(c) = game.lists.client(client) else {
            return;
        };
        let (state, room) = (c.state, c.room);
        let Some(player) = c.player else {
            return;
        };
        self.sys.hooks.x.environment_refresh_items(player);
        // `0x0055FDE0(…, 1)`: the inventory pass `0x0055DBC0(0)` (no
        // message) and the owner refresh `0x00621000(player, 1)`: queued
        // for update with unit +0xC8 bits 0 and 1, so the player's unit
        // update sends 0x47 and 0x48 after this tick's 0x53
        // (`items/inventory-moves.md` §6.1; recorded `a2-super-fangskin`
        // frames 6 and 74).
        let _ = game.lists.queue_update(player);
        if let Some(r) = self.sys.units.get_mut(player) {
            r.flags2 |= 0x3;
        }
        let in_act = room
            .and_then(|r| game.lists.room(r))
            .is_some_and(|r| r.act == act);
        if state != client_state::IN_GAME || !in_act {
            return;
        }
        if let Some(a) = game.lists.act(act) {
            let m = a.environment.message();
            self.sys.hooks.x.send(player, &m);
        }
    }

    /// Step 9 `0x0061A790` (`rooms.md` §7.2).
    fn room_inactivity(&mut self, game: &mut Game, room: RoomId) -> u32 {
        match self.sys.hooks.drlg.room_inactivity(game, room) {
            Ok(n) => n,
            Err(e) => {
                self.sys.hooks.errors.push(e);
                0
            }
        }
    }

    /// Step 9 `0x0061A3F0` (`rooms.md` §8.1).
    fn act_allows_room_removal(&mut self, _: &mut Game, act: u8, room: RoomId) -> bool {
        match self.sys.hooks.drlg.allows_removal(act, room) {
            Ok(b) => b,
            Err(e) => {
                self.sys.hooks.errors.push(e);
                false
            }
        }
    }

    /// Step 9, the rest of `0x0061A910` (`rooms.md` §8.2).
    fn room_deactivated(&mut self, game: &mut Game, act: u8, room: RoomId) {
        let r = self
            .sys
            .hooks
            .drlg
            .remove_active_room(&mut game.lists, act, room);
        self.log(r);
    }

    /// Step 9 `0x005433F0` (`units.md` §3.3) on the inactive store
    /// ([`ActionSim::compress`]; only warp tiles while the store is off).
    fn compress_unit(&mut self, game: &mut Game, unit: UnitId) {
        self.compress(game, unit);
    }

    /// Step 10 `0x0061AA20` (`levels.md` §9.2).
    fn free_inactive_rooms(&mut self, _: &mut Game, act: u8) {
        let r = self.sys.hooks.drlg.free_inactive_rooms(act);
        self.log(r);
    }

    /// Per-client update (`tick.md` §6.5, `0x0053A5D0`): with the path
    /// provider on, a player's movement messages (`pathing.md` §10 rules
    /// 2–3, [`crate::wiring::path::walk::update_messages`]) and a
    /// monster's mode message (`intents-events.md` §7.3 rule 2 step 2,
    /// [`View::monster_update`]); objects: the object update pass
    /// `0x00581AD0` (`objects.md` §14, [`View::object_update`]) to the
    /// client's player. Without the provider no unit has a path record,
    /// so no player or monster message is built.
    fn send_unit_update(&mut self, game: &mut Game, client: ClientId, unit: UnitId) {
        let s = &mut self.sys;
        let mut v = View::of(&mut s.units, &mut s.stats, &s.data, &mut s.hooks);
        // §7.1 rule 2.1: a unit not yet announced (unit flag 0x10), other
        // than the client's player: a missile sends nothing at all; any
        // other type its add messages (§7.2) first. A monster's are sent
        // by its update ([`View::monster_update`], which also reads
        // "announced" for its step 8).
        let receiver = game.lists.client(client).and_then(|c| c.player);
        // §6.3 part 1 (`inventory-moves.md`): an item in the walk is
        // announced at its place in the queue order; the host does it
        // from this mark.
        if v.h.item_marks {
            if let (Some(p), Some(r)) = (receiver, v.units.get(unit)) {
                if r.ty == UnitType::Item {
                    let mut m = [super::GROUND_ITEM_MARK; 5];
                    m[1..].copy_from_slice(&r.guid.to_le_bytes());
                    v.h.x.send(p, &m);
                }
            }
        }
        let new = v
            .units
            .get(unit)
            .filter(|r| r.flags & crate::units::record::flags::SEED_SET != 0)
            .map(|r| r.ty);
        if let (Some(ty), Some(p)) = (new, receiver) {
            if p != unit {
                match ty {
                    UnitType::Missile => return,
                    UnitType::Monster => {}
                    _ => v.add_messages(game, p, unit),
                }
            }
        }
        let is_player = v.units.get(unit).is_some_and(|r| r.ty == UnitType::Player);
        // §3.5 rule 6 / §7.3 rule 2 step 8: the changed-state messages of
        // a unit that is not new to the client (a player's: below).
        if let (Some(p), None, false) = (receiver, new, is_player) {
            v.state_change_messages(p, unit);
        }
        if is_player {
            // §7.3 rule 1 (`0x00580860`): steps 1 and 3 (the path part),
            // step 4's soft hit, then step 5 (any state-changed bit, whether announced or
            // not) and step 7.
            if v.h.paths.is_some() {
                crate::wiring::path::walk::update_messages(&mut v, game, client, unit);
                // Step 4: the soft hit (0x8000 → 0x0D).
                crate::wiring::path::walk::soft_hit_message(&mut v, game, client, unit);
            }
            if let Some(p) = receiver {
                // Step 4 (`0x00571CD0`, §7.9 rule 2): the pending event
                // records, e.g. the 0xA5 landing message of a failed
                // Whirlwind start (`bodies-2b.md` §8.10).
                v.send_event_records(game, p, unit);
                // A unit still new to the client keeps its join order; so
                // does one without item messages pending (update bit 0).
                v.h.capture_tail =
                    new.is_none() && v.units.get(unit).is_some_and(|r| r.flags2 & 1 != 0);
                v.state_change_messages(p, unit);
                v.player_stat_sends(p, unit);
                v.h.capture_tail = false;
            }
            return;
        }
        if game
            .lists
            .unit(unit)
            .is_some_and(|e| e.ty == UnitType::Object)
        {
            if let Some(receiver) = game.lists.client(client).and_then(|c| c.player) {
                v.object_update(game, receiver, unit);
                // `objects.md` §14 rule 2: flag 0x400 → `0x00571740`.
                if let Some(m) = crate::units::sound::sound_message(game, unit, receiver) {
                    v.h.x.send(receiver, &m);
                }
                // §7.3 rule 3: always `0x00571CD0` (§7.9 rule 2).
                v.send_event_records(game, receiver, unit);
            }
            return;
        }
        if v.h.paths.is_none() {
            return;
        }
        if game
            .lists
            .unit(unit)
            .is_some_and(|e| e.ty == UnitType::Monster)
        {
            v.monster_update(game, client, unit);
        }
    }

    /// Per-client update (`tick.md` §6 rule 5, after the unit updates):
    /// the player's stat-change messages, the flush `0x006258D0` of its
    /// changed-stat array (`stat-lists.md` §11 rule 2,
    /// [`vitals_sync::mod_stat_messages`]); then, only when the player's
    /// flag-ex (+0xC8) bit 21 is set, the inventory refresh
    /// `0x0055DF00(…, 1, 1)` → `0x0055DBC0` (`intents-events.md` §8.3),
    /// whose send ends with S→C 0x48 (type 0, arg 0, the player's GUID;
    /// `inventory.md` §5.7 step 8). `0x0055F4F0` is empty in 1.14d.
    ///
    /// PROVISIONAL (REC-405; d2rs-own, unverified): the refresh's item and
    /// skill steps (§5.7 steps 1–7) are not run here: they belong to the
    /// host's inventory model, which the tick hooks do not hold. Its 0x48
    /// is sent at the spec's place.
    fn client_update_messages(&mut self, game: &mut Game, client: ClientId) {
        let Some(p) = game.lists.client(client).and_then(|c| c.player) else {
            return;
        };
        let Some(r) = self.sys.units.get(p) else {
            return;
        };
        let (guid, refresh) = (r.guid, r.flags2 & INVENTORY_REFRESH_EX != 0);
        let msgs = vitals_sync::mod_stat_messages(&self.sys.stats.mod_values(p));
        for m in &msgs {
            self.send_after_items(p, m);
        }
        if refresh {
            self.sys.hooks.x.send(
                p,
                &crate::items::moves::layouts::relator2(UnitType::Player as u8, 0, guid),
            );
        }
    }

    /// Per-client update (`tick.md` §6.5, `0x0053FC20`): with arena flag
    /// 0x400 raised, S→C 0x65 for the client's player when its arena
    /// record's flag is set (`intents-events.md` §7.6 rule 5).
    fn arena_sync(&mut self, game: &mut Game, client: ClientId) {
        let Some(st) = self.sys.hooks.arena.as_ref().filter(|s| s.flag_400) else {
            return;
        };
        let Some(p) = game.lists.client(client).and_then(|c| c.player) else {
            return;
        };
        let Some(&(score, true)) = st.records.get(&p) else {
            return;
        };
        let Some(guid) = self.sys.units.get(p).map(|r| r.guid) else {
            return;
        };
        let m = crate::units::messages::player_kill_count(guid, score as u16);
        self.send_after_items(p, &m);
    }

    /// Step 6, last (`0x0053FAE0`): arena flag 0x400 := 0.
    fn clear_arena_flag(&mut self, _: &mut Game) {
        if let Some(st) = self.sys.hooks.arena.as_mut() {
            st.flag_400 = false;
        }
    }

    /// Step 6 (`0x00553220`, `intents-events.md` §7.5): the flag part of
    /// the room clean-up ([`View::room_cleanup`]) and unit flag 0x400, the
    /// sound slot (step 3, [`crate::units::sound`]). A player's slot is
    /// read by its unit update `0x00580860` after the item messages
    /// (`cube.md` §8 rule 3), which the server runs after the tick
    /// (`d2-server` `handlers::items::moves::update_pass`); that pass
    /// clears it.
    fn unit_update(&mut self, game: &mut Game, unit: UnitId) {
        if game
            .lists
            .unit(unit)
            .is_some_and(|e| e.ty != UnitType::Player)
        {
            game.sounds.clear(unit);
        }
        let s = &mut self.sys;
        View::of(&mut s.units, &mut s.stats, &s.data, &mut s.hooks).room_cleanup(unit);
    }

    /// Per-client update (`tick.md` §6 rule 5): the player's room differs
    /// from the client's. When the two rooms' level ids differ, quest
    /// event 3 CHANGEDLEVEL `0x00543B90(game, from, to, player)`
    /// (`world/quests.md` §4.1) runs first, on the host's lent quest
    /// control ([`super::objects::QuestObjectHost::changed_level`]; none
    /// lent: nothing); then the room switch `0x00537B50` to the player's
    /// room ([`View::room_switch`], `intents-events.md` §7.8).
    ///
    /// TODO(tick.md §6 rule 5): the town-leave refresh `0x00537340`
    /// (`VendorDesk::level_changed`) after quest event 3: the vendor
    /// records are the host's and are not lent to the tick.
    fn client_level_change(&mut self, game: &mut Game, client: ClientId) {
        let Some(c) = game.lists.client(client) else {
            return;
        };
        let (player, client_room) = (c.player, c.room);
        let new = player
            .and_then(|p| game.lists.unit(p))
            .and_then(|u| u.room());
        let level =
            |r: Option<RoomId>, h: &ActionHooks<X>| r.and_then(|r| h.drlg.level_id(game, r));
        let (from, to) = (
            level(client_room, &self.sys.hooks),
            level(new, &self.sys.hooks),
        );
        if let (Some(_), Some(to)) = (player, to) {
            if from != Some(to) {
                crate::cov!(Level, to, 0);
            }
        }
        if let (Some(p), Some(from), Some(to)) = (player, from, to) {
            if from != to {
                self.with(game, |g, v| {
                    if let Some(mut host) = v.h.quest_host.take() {
                        host.changed_level(g, v, p, from, to);
                        v.h.quest_host = Some(host);
                    }
                });
            }
        }
        self.with(game, |g, v| v.room_switch(g, client, new));
    }

    /// Step 5 (`0x0061A460`, `tick.md` §6 rule 6): the client's room is
    /// ready ([`View::client_room_ready`]).
    fn client_room_ready(&mut self, game: &mut Game, client: ClientId) -> bool {
        self.with(game, |g, v| v.client_room_ready(g, client))
    }

    /// Step 5, after state 4 (`tick.md` §6 rule 4): the inventory refresh
    /// `0x0055DF00` → `0x0055DBC0` (`intents-events.md` §8.3, the second
    /// recorded 0x48), whose send ends with S→C 0x48 (type 0, arg 0, the
    /// player's GUID; `inventory.md` §5.7 step 8). PROVISIONAL (REC-405):
    /// the pass's item steps are not run here.
    fn refresh_inventory(&mut self, game: &mut Game, client: ClientId) {
        let Some(p) = game.lists.client(client).and_then(|c| c.player) else {
            return;
        };
        let Some(guid) = self.sys.units.get(p).map(|r| r.guid) else {
            return;
        };
        let m = crate::items::moves::layouts::relator2(UnitType::Player as u8, 0, guid);
        self.sys.hooks.x.send(p, &m);
    }

    /// Step 5: S→C 0x04 LoadComplete (`0x0053B320(client, 4)`, `tick.md`
    /// §6 rule 6) to the client's player.
    fn send_load_complete(&mut self, game: &mut Game, client: ClientId) {
        if let Some(p) = game.lists.client(client).and_then(|e| e.player) {
            self.sys
                .hooks
                .x
                .send(p, &crate::units::messages::LOAD_COMPLETE);
        }
    }

    /// Step 5, the join sequence (`intents-events.md` §8.3, REC-401 and
    /// REC-406 settled; `flows/game-join.md` §3 r2), J = the joiner:
    /// `0x0052C410`: for each other client C with a player (any state,
    /// client-list order) S→C 0x5B of P(C) to J, 0x5B of P(J) to C, then
    /// to J one 0x8E CorpseAssign per corpse of P(C); then 0x5B of P(J)
    /// to J. `0x0053FC70`: to J one 0x65 per state-4 client's player
    /// (list order). `0x0055B620`: to J one 0x8D (GUID, party word; d2rs
    /// has no parties) per player unit without state 7 (hash-bucket
    /// order). Then the join 0x5A (code 2, when the name has a NUL in its
    /// 16 bytes) to every state-4 client, the joiner included. A
    /// single-player join sends 0x5B, 0x65, 0x8D, 0x5A to the one client.
    // PROVISIONAL (no REC; `docs/handoff/pc1-data.md` Step 4): the 0x8E
    // flag byte (1 = assign, the counterpart of the corpse-take's flag 0)
    // is not read from `0x0053DFB0`'s caller.
    fn join_sequence(&mut self, game: &mut Game, client: ClientId) {
        use crate::units::lists::client_state::IN_GAME;
        use crate::units::messages as m;
        let Some(j) = game.lists.client(client).and_then(|e| e.player) else {
            return;
        };
        let clients: Vec<(ClientId, Option<UnitId>, u32)> = game
            .lists
            .clients()
            .into_iter()
            .filter_map(|c| game.lists.client(c).map(|e| (c, e.player, e.state)))
            .collect();
        let joined = |a: &Self, p: UnitId| -> Option<Vec<u8>> {
            let r = a.sys.units.get(p)?;
            let level = a.sys.stats.unit_total(p, 12, 0) as u16;
            let name = a
                .sys
                .hooks
                .session
                .names
                .get(&p)
                .copied()
                .unwrap_or([0; 16]);
            Some(m::player_joined(
                r.guid,
                r.class as u8,
                &name,
                level,
                m::NO_PARTY,
            ))
        };
        let Some(own) = joined(self, j) else {
            return;
        };
        let guid_of = |a: &Self, p: UnitId| a.sys.units.get(p).map_or(0, |r| r.guid);
        // `0x0052C410`.
        for &(c, pc, _) in &clients {
            let Some(pc) = pc else { continue };
            if c == client {
                continue;
            }
            if let Some(theirs) = joined(self, pc) {
                self.sys.hooks.x.send(j, &theirs);
            }
            self.sys.hooks.x.send(pc, &own);
            let owner = guid_of(self, pc);
            let corpses: Vec<u32> = self
                .sys
                .hooks
                .death
                .owners
                .iter()
                .filter(|&(_, &o)| o == owner)
                .map(|(&u, _)| guid_of(self, u))
                .collect();
            for g in corpses {
                let mut b = vec![0x8E, 1];
                b.extend_from_slice(&owner.to_le_bytes());
                b.extend_from_slice(&g.to_le_bytes());
                self.sys.hooks.x.send(j, &b);
            }
        }
        self.sys.hooks.x.send(j, &own);
        // `0x0053FC70`: one kill count per in-game client's player.
        for &(_, pc, state) in &clients {
            if let (Some(pc), true) = (pc, state == IN_GAME) {
                let kills = m::player_kill_count(guid_of(self, pc), 0);
                self.sys.hooks.x.send(j, &kills);
            }
        }
        // `0x0055B620`: one 0x8D per player unit without state 7.
        for q in game.lists.units_of_type(UnitType::Player) {
            if self.sys.stats.has_state(q, super::death::STATE_PLAYERBODY) {
                continue;
            }
            let party = m::assign_player_to_party(guid_of(self, q), m::NO_PARTY);
            self.sys.hooks.x.send(j, &party);
        }
        // The join 0x5A to every state-4 client.
        let name = self
            .sys
            .hooks
            .session
            .names
            .get(&j)
            .copied()
            .unwrap_or([0; 16]);
        if name.contains(&0) {
            let e = m::player_event(2, &name);
            for &(_, pc, state) in &clients {
                if let (Some(pc), true) = (pc, state == IN_GAME) {
                    self.sys.hooks.x.send(pc, &e);
                }
            }
        }
    }
}
