// Spec: specs/sim/units.md §3, §4.1, §4.3, §4.5, §4.6, §5, §6; specs/monsters/init.md §5, §22; specs/monsters/umod-callbacks.md §2; specs/formats/animdata.md §3–§5; specs/sim/stat-lists.md §4, §8, §9; specs/monsters/ai.md §1; specs/missiles/missiles.md §R3
//! The unit side of the wiring: the unit hooks of [`ActionHooks`] (the
//! missile class handler for missile events, the AI think and reset for
//! monster events 2 and 10, the state-54 rule before a think is
//! scheduled, the town test, the combat list drop, the kind frees, the
//! skill events 5 / 8 / 9 through [`Pending::skill_event`], the player
//! action frame through [`Pending::action_frame`], the AnimData record
//! of a unit's mode (`formats/animdata.md` §5) and the monster death
//! start through [`Pending::monster_death_start`]; with a lent monster
//! world ([`super::monsters`]): the monster type init of the allocator,
//! the monster state's part of a free, event 7 and the mode change's
//! umod callbacks), and
//! the unit-field helpers of [`View`] the other adapters share (stats,
//! states, state lists, seeds).

use crate::game::Game;
use crate::missiles;
use crate::monsters::ai;
use crate::rng::Seed;
use crate::stats::lists::RemoveCallback;
use crate::stats::states::state;
use crate::stats::{ListId, StatHost, StatLists};
use crate::tick::events::event;
use crate::units::hooks::{Sim, UnitHooks};
use crate::units::lifecycle::{AllocRequest, LifecycleHooks};
use crate::units::modes::UnitError;
use crate::units::modes::MONSTER_MODES;
use crate::units::record::{flags2, AnimRecord, ANIM_EVENTS};
use crate::units::{UnitId, UnitType};

use super::combat::HIRELING_CLASSES;
use super::monsters::umod_mode;
use super::{ActionHooks, Pending, SkillEvent, View, WiringError};

/// Stat-list state of `justhit` (`missiles.md` §R5 step 6.1).
pub const STATE_JUSTHIT: u16 = 86;
/// State 92 (`death_delay`), cleared for players by `0x005544B0`.
pub const STATE_DEATH_DELAY: u16 = 92;

impl<X: Pending> StatHost for ActionHooks<X> {
    /// §8.2 rule 6: queue the callbacks this wiring runs after the expiry
    /// walk ([`UnitHooks::lists_expired`]): the default one and the shrine
    /// ones. The others are run by their skill bodies.
    fn list_removed(
        &mut self,
        _lists: &mut StatLists,
        unit: UnitId,
        state: u32,
        _list: ListId,
        callback: RemoveCallback,
    ) {
        use crate::world::objects::shrines::{SKILL_REMOVE, STAMINA_REMOVE};
        if matches!(callback.0, 0x0056_E900 | SKILL_REMOVE | STAMINA_REMOVE) {
            self.removed_lists.push((unit, state, callback.0));
        }
    }
}

impl<X: Pending> ActionHooks<X> {
    /// `0x0066A9B0` (`animdata.md` §5): the record of the COF name the
    /// composer ([`Pending::anim_name`]) builds for the unit's type,
    /// class and mode, looked up by §4; a name the file lacks gets the
    /// default record (§3). `None` when no table is loaded, the unit has
    /// no record, or the composer gives no name.
    fn anim_lookup(
        &mut self,
        sim: &Sim<'_>,
        unit: UnitId,
    ) -> Option<d2_formats::animdata::AnimRecord> {
        let data = self.anim_data.clone()?;
        let r = sim.units.get(unit)?;
        let name = self.x.anim_name(unit, r.ty, r.class, r.mode)?;
        match data.record(&name) {
            Ok(rec) => Some(rec.clone()),
            Err(e) => {
                self.errors.push(WiringError::AnimData(e));
                None
            }
        }
    }
}

/// The fields `units.md` §4.2 reads from an AnimData record: frames,
/// byte +0x0F (the speed's high byte, read as event index −1 by the
/// variants) and the 144 event bytes.
pub fn anim_record(r: &d2_formats::animdata::AnimRecord) -> AnimRecord {
    let mut events = [0u8; ANIM_EVENTS];
    events.copy_from_slice(&r.events[..ANIM_EVENTS]);
    AnimRecord {
        frames: r.frames,
        byte_0f: (r.speed >> 24) as u8,
        events,
    }
}

impl<X: Pending> UnitHooks for ActionHooks<X> {
    /// Runs the queued remove callbacks of the lists the expiry walk
    /// freed (`stat-lists.md` §8.2 rule 6, `skills/bodies.md` §2.8).
    // PROVISIONAL (REC-263; d2rs-own, unverified): the bodies of the shrine
    // callbacks `0x00583BD0` / `0x00583A40` are unwritten. Each runs the
    // default (state off) and the stamina one also clamps stamina to its
    // maximum (the shrine set stamina to 2v on the list); the skill one's
    // skill refresh has nothing to refresh here (levels read the stat).
    fn lists_expired(&mut self, sim: &mut Sim<'_>, unit: UnitId) {
        use crate::world::objects::shrines::STAMINA_REMOVE;
        for (u, state, cb) in std::mem::take(&mut self.removed_lists) {
            let t = sim.stats.toggle_state(u, state, false);
            if let (Some(d), Some(r)) = (t.disguise, sim.units.get_mut(u)) {
                if d {
                    r.flags2 |= flags2::DISGUISE;
                } else {
                    r.flags2 &= !flags2::DISGUISE;
                }
            }
            if cb == STAMINA_REMOVE {
                sim.stats.clamp_to_max(self, u);
            }
        }
        let _ = unit;
    }
    /// `0x00580EC0`: the death penalties at `0x00580F59`
    /// (`vitals.md` §4.6, [`super::death`]).
    fn player_death(&mut self, sim: &mut Sim<'_>, unit: UnitId) {
        self.death_penalties(sim, unit);
    }
    /// `0x0057FCA0`: the corpse creation `0x0057F700` at `0x0057FD1C`
    /// (`vitals.md` §4.7 rule 1, [`super::death`]), then `0x00575BC0`
    /// at `0x0057FD25` in every game type (`hirelings-2.md` §15 rule 1):
    /// queued for the host that holds the hireling lists
    /// ([`ActionHooks::owner_deaths`]).
    fn player_corpse(&mut self, sim: &mut Sim<'_>, unit: UnitId) {
        self.corpse_creation(sim, unit);
        if let Some(q) = self.owner_deaths.as_mut() {
            q.push(unit);
        }
    }
    /// `0x0057FB70` ([`super::death`]; the experience it returns is not
    /// read by the 0x16 caller).
    fn player_corpse_pickup(&mut self, sim: &mut Sim<'_>, player: UnitId, corpse: UnitId) -> bool {
        self.corpse_pickup(sim, player, corpse).is_some()
    }
    /// `0x00620F00`: the AnimData record of the unit's mode
    /// (`units.md` §4.1, `animdata.md` §5).
    fn anim_record(&mut self, sim: &Sim<'_>, unit: UnitId) -> Option<AnimRecord> {
        self.anim_lookup(sim, unit).map(|r| anim_record(&r))
    }

    /// `0x00623F50` (`units.md` §4.3) from the record's speed (+0x0C).
    fn anim_rate(&mut self, sim: &Sim<'_>, unit: UnitId) -> i16 {
        let speed = self.anim_lookup(sim, unit).map(|r| r.speed);
        self.x.anim_rate(unit, speed)
    }

    /// The velocity half of `0x00623F50` for monsters with the path
    /// provider ([`crate::wiring::path::monsters`]).
    fn anim_velocity(&mut self, sim: &mut Sim<'_>, unit: UnitId) {
        self.monster_mode_velocity(sim, unit);
    }

    /// The path part of `0x005A7C20` ([`crate::wiring::path::monsters`]);
    /// the requested mode is kept for the start function.
    fn monster_mode_bookkeeping(&mut self, sim: &mut Sim<'_>, unit: UnitId, mode: u32) {
        self.monster_request = mode;
        self.monster_path_setup(sim, unit, mode);
    }

    /// `0x00623B10` (`units.md` §4.3).
    fn frame_bonus(&mut self, _: &Sim<'_>, unit: UnitId) -> i32 {
        self.x.frame_bonus(unit)
    }

    fn has_path(&mut self, _: &Sim<'_>, unit: UnitId) -> bool {
        self.path_has(unit)
    }

    /// The provider's static path record (`path-placement.md` §2.1).
    fn static_position(&self, unit: UnitId) -> Option<(i32, i32)> {
        match self.paths.as_ref()?.record(unit)? {
            crate::path::UnitPath::Static(s) => Some((s.x, s.y)),
            crate::path::UnitPath::Dynamic(_) => None,
        }
    }

    /// Player event 0 in modes 2, 3, 6, 19: the player step `0x00580C20`
    /// (`pathing.md` §9.2) with the path provider; the step result (2:
    /// stopped, the ENDANIM handler follows). Without the provider: the
    /// trait default (1, nothing moves).
    fn player_movement_step(&mut self, sim: &mut Sim<'_>, unit: UnitId, a1: u32, a2: u32) -> u32 {
        let _ = (a1, a2);
        if self.paths.is_none() {
            return 1;
        }
        let mut v = View::of(sim.units, sim.stats, sim.data, self);
        crate::wiring::path::walk::player_step(&mut v, sim.game, unit)
    }

    /// Player event 0 in attack, cast and skill modes (`0x00580460`,
    /// `units.md` §4.5), through [`Pending::action_frame`].
    fn player_action_frame(&mut self, sim: &mut Sim<'_>, unit: UnitId, a1: u32, a2: u32) -> u32 {
        X::action_frame(self, sim, unit, a1, a2)
    }

    /// Monster mode functions (`units.md` §4.6): the start and event
    /// functions of rules 5–14 ([`crate::wiring::path::monsters`]); the
    /// death start `0x005A6FF0` goes to [`Pending::monster_death_start`]
    /// with the mode change's target; DT's event functions `0x005A7350` /
    /// `0x005A72B0` end the death in mode 12 (`intents-events.md` §7.7
    /// rule 3, [`super::unit_update::death_function`]); every other
    /// function keeps the default (started, nothing done).
    fn monster_mode_function(&mut self, sim: &mut Sim<'_>, unit: UnitId, address: u32) -> bool {
        if let Some(started) = self.monster_motion_function(sim, unit, address) {
            return started;
        }
        if address == MONSTER_MODES[0].start {
            let target = self.mode_target;
            return X::monster_death_start(self, sim, unit, target);
        }
        super::unit_update::death_function(self, sim, unit, address);
        true
    }

    /// `0x0057C980`: the unit's own entries leave its combat list
    /// (`damage.md` §3 step 3: entries are (attacker, defender) records).
    fn drop_combat_entries(&mut self, sim: &mut Sim<'_>, unit: UnitId) {
        let Some(e) = sim.game.lists.unit(unit) else {
            return;
        };
        let id = (e.ty, e.guid);
        if let Some(list) = self.combat_lists.get_mut(&unit) {
            list.retain(|c| c.attacker != id);
        }
    }

    /// `0x0061AB00` on the unit's room.
    fn room_flag(&mut self, sim: &Sim<'_>, unit: UnitId) -> bool {
        sim.game
            .lists
            .unit(unit)
            .and_then(|e| e.room())
            .is_some_and(|r| self.drlg.in_town(sim.game, r))
    }

    /// `0x005544B0(unit, 0)` before a think is scheduled on a monster with
    /// state 54 (`tick.md` §5.2 rule 4, `ai.md` §1.1): state 54 off, the
    /// monster's type-2 events cancelled.
    fn uninterruptable_check(&mut self, sim: &mut Sim<'_>, unit: UnitId) {
        let mut v = View::of(sim.units, sim.stats, sim.data, self);
        v.set_state(unit, state::UNINTERRUPTABLE as u16, false);
        sim.game
            .timers
            .cancel_unit_events(unit, event::AI_THINK, None);
    }

    /// Event 2 `0x005B1740` (`ai.md` §2). The freeze drop of `tick.md`
    /// §5.6 already ran in the unit dispatch.
    fn ai_think(&mut self, sim: &mut Sim<'_>, unit: UnitId, _: u32, _: u32) {
        let Some(mut store) = self.ai.take() else {
            self.errors.push(WiringError::Reentrant("ai"));
            return;
        };
        let t = self.tables.clone();
        let info = self.ai_info;
        {
            let mut v = View::of(sim.units, sim.stats, sim.data, self);
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
            ai::think(sim.game, &mut cx, unit);
        }
        self.ai = Some(store);
    }

    /// The mode-set sites of the umod dispatcher (`umod-callbacks.md`
    /// §2 rules 1–2) on the lent monster world; without one, nothing.
    fn monster_umods(&mut self, sim: &mut Sim<'_>, unit: UnitId, mode: u8) {
        self.run_umods(sim, unit, None, mode);
    }

    /// Event 7 `0x005A4370` → the umod dispatcher in mode 2 (`init.md`
    /// §22) on the lent monster world ([`super::monsters`]); without one,
    /// nothing (the trait default). The unit dispatch already ran the
    /// handler-table checks and the frozen-monster drop (`tick.md` §5.6).
    fn monster_umod(&mut self, sim: &mut Sim<'_>, unit: UnitId, _: u32, _: u32) {
        self.run_umods(sim, unit, None, umod_mode::EVENT7);
    }

    /// Event 10 `0x005A7F70` → `0x00573120` (`ai.md` §1; monster data).
    fn ai_reset(&mut self, _: &mut Sim<'_>, unit: UnitId, _: u32, _: u32) {
        self.x.ai_reset(unit);
    }

    /// Event 5 `0x0056D790` (`stat-lists.md` §10.2): the skills'
    /// active-state function, through [`Pending::skill_event`].
    fn active_state(&mut self, sim: &mut Sim<'_>, unit: UnitId, f: u16, skill: u32, a2: u32) {
        X::skill_event(
            self,
            sim,
            SkillEvent::ActiveState {
                unit,
                f,
                skill,
                arg2: a2,
            },
        );
    }

    /// Event 8 `0x0056FCB0` (`use.md` §7), through [`Pending::skill_event`].
    fn periodic_skills(&mut self, sim: &mut Sim<'_>, unit: UnitId, a1: u32, a2: u32) {
        X::skill_event(
            self,
            sim,
            SkillEvent::Periodic {
                unit,
                arg1: a1,
                arg2: a2,
            },
        );
    }

    /// Event 9 `0x0056FE40` after its checks (`stat-lists.md` §10.3), through
    /// [`Pending::skill_event`].
    fn apply_item_aura(
        &mut self,
        sim: &mut Sim<'_>,
        unit: UnitId,
        a1: u32,
        skill: u32,
        level: i32,
    ) {
        X::skill_event(
            self,
            sim,
            SkillEvent::ItemAura {
                unit,
                arg1: a1,
                skill,
                level,
            },
        );
    }

    /// Object events (`units.md` §6.4) on the object state
    /// ([`super::objects`]); a game without one keeps the default.
    fn object_event(&mut self, sim: &mut Sim<'_>, unit: UnitId, event: u8) {
        View::of(sim.units, sim.stats, sim.data, self).object_event(sim.game, unit, event);
    }

    /// Missile events (`0x005ADBB0`, `missiles.md` §R3).
    fn missile_do(&mut self, sim: &mut Sim<'_>, unit: UnitId) {
        let Some(mut store) = self.missiles.take() else {
            self.errors.push(WiringError::Reentrant("missiles"));
            return;
        };
        let t = self.tables.clone();
        {
            let mut v = View::of(sim.units, sim.stats, sim.data, self);
            let mut cx = missiles::Ctx {
                tables: &t.missiles,
                store: &mut store,
                world: &mut v,
            };
            missiles::class_handler(sim.game, &mut cx, unit);
        }
        self.missiles = Some(store);
    }
}

impl<X: Pending> LifecycleHooks for ActionHooks<X> {
    fn request_act_change(&mut self, player: UnitId, level: u32, arg: u32) {
        self.act_changes.push((player, level, arg));
    }
    /// The monster type init `0x00574250` (`init.md` §5, `units.md` §3.1
    /// table: the allocator's per-kind init of a monster) on the lent
    /// monster world ([`super::monsters`]); the object data and init
    /// `0x0054F5D0` of an object on the object state
    /// ([`View::object_init`], `objects.md` §3); other kinds, and a game
    /// without the lent world or the object state, keep the default
    /// (nothing).
    fn init_kind(&mut self, sim: &mut Sim<'_>, unit: UnitId, req: &AllocRequest) {
        // The init's room is the allocation's (r7.2) until step 8.
        self.alloc_rooms.push((unit, req.room));
        if req.ty == UnitType::Monster {
            self.with_monster_world(|w, h| w.type_init(sim, h, unit));
        } else if req.ty == UnitType::Object {
            // Inside `View::allocate`: run once its seed step is back in
            // the hooks (the init may allocate and draw itself).
            if let Some(q) = self.deferred_inits.as_mut() {
                q.push(unit);
                return;
            }
            View::of(sim.units, sim.stats, sim.data, self).object_init(sim.game, unit);
        }
    }

    /// The mercenary's creation (`npc.md` §7.3 step 7): a monster of
    /// `class` in the room of `near`, a few subtiles beside it.
    // d2rs-own, unverified: the offset (+2, +2) stands in for the
    // placement `hirelings.md` §3.1 leaves to the path code's free-spot
    // search; the allocation's path part validates the spot.
    fn spawn_near(
        &mut self,
        sim: &mut Sim<'_>,
        near: UnitId,
        class: u32,
        mode: u8,
    ) -> Option<UnitId> {
        let room = sim.game.lists.unit(near)?.room()?;
        let (x, y) = self.path_position(near);
        let req = AllocRequest {
            ty: UnitType::Monster,
            class,
            room: Some(room),
            add: true,
            fixed_guid: None,
            mode: u32::from(mode),
            allied: false,
        };
        View::of(sim.units, sim.stats, sim.data, self).allocate(sim.game, &req, x + 2, y + 2)
    }

    /// Step 8 linked the unit: its room is the list's from now on.
    fn added(&mut self, _: &mut Sim<'_>, unit: UnitId) {
        self.alloc_rooms.retain(|&(u, _)| u != unit);
    }

    /// The per-kind state of the action modules leaves with the unit:
    /// AI control (`AiStore::remove`), missile data, combat list; then
    /// the lent monster world's part (monster data, minion list, owner
    /// link); an object's object data.
    fn free_kind(&mut self, sim: &mut Sim<'_>, unit: UnitId) {
        let (ty, mode) = sim
            .units
            .get(unit)
            .map_or((None, 0), |r| (Some(r.ty), r.mode));
        self.path_free(unit, ty, mode);
        if let Some(ai) = self.ai.as_mut() {
            ai.remove(unit);
        }
        if let Some(m) = self.missiles.as_mut() {
            m.remove(unit);
        }
        self.combat_lists.remove(&unit);
        self.handlers.remove(&unit);
        if let Some(st) = self.objects.as_mut() {
            st.control.data.remove(&unit);
        }
        self.with_monster_world(|w, _| w.forget(unit));
    }
}

impl<X: Pending> View<'_, X> {
    /// Records an error of a unit operation.
    pub fn unit_error(&mut self, e: UnitError) {
        self.h.errors.push(WiringError::Unit(e));
    }

    /// The unit seed (unit +0x20). A unit without a record gets a scratch
    /// seed and an error (API misuse).
    pub fn seed(&mut self, u: UnitId) -> &mut Seed {
        if self.units.get(u).is_none() {
            self.h
                .errors
                .push(WiringError::Unit(UnitError::UnknownUnit(u)));
            self.h.orphan_seed = Seed::init();
            return &mut self.h.orphan_seed;
        }
        &mut self.units.get_mut(u).expect("checked").seed
    }

    /// Unit getter `0x00625480(unit, stat, 0)`.
    pub fn stat(&self, u: UnitId, s: u16) -> i32 {
        self.stats.unit_total(u, s, 0)
    }

    /// Unit set `0x00627260(unit, stat, value, 0)`.
    pub fn set_base(&mut self, u: UnitId, s: u16, value: i32) {
        self.stats.unit_set(&mut *self.h, u, s, value, 0);
    }

    /// Set a stat of a list (`0x006270B0`, layer 0).
    pub fn set_list_stat(&mut self, l: ListId, s: u16, value: i32) {
        self.stats.set(&mut *self.h, l, s, value, 0, None);
    }

    /// State toggle `0x00625A70` (`stat-lists.md` §9.2) with the disguise
    /// bit of unit +0xC8.
    pub fn set_state(&mut self, u: UnitId, s: u16, on: bool) {
        let t = self.stats.toggle_state(u, u32::from(s), on);
        if let (Some(d), Some(r)) = (t.disguise, self.units.get_mut(u)) {
            if d {
                r.flags2 |= flags2::DISGUISE;
            } else {
                r.flags2 &= !flags2::DISGUISE;
            }
        }
    }

    /// The unit's stat list of `state` (`0x006256B0`).
    pub fn state_list(&self, u: UnitId, s: u16) -> Option<ListId> {
        let r = self.stats.unit_list(u)?;
        self.stats.list_of_state(r, u32::from(s))
    }

    /// `stat` of the unit's list of `state` (its own base value).
    pub fn state_stat(&self, u: UnitId, s: u16, st: u16) -> Option<i32> {
        let l = self.state_list(u, s)?;
        Some(self.stats.base(l, st, 0))
    }

    /// A plain stat list for `state` attached to the unit: allocation
    /// `0x006251F0` with the owner's type and GUID, expire `0x00627440`
    /// (sets NEWLENGTH when > 0), state field, attach `0x00626E10`.
    ///
    /// TODO(stat-lists.md §4, §8.1): the callers' allocation flags and the
    /// attach `reset` argument are not stated for state lists; flags 0
    /// and reset = 1 (no DYNAMIC) are used. Without an owner the unit's
    /// own type and GUID are used.
    pub fn create_state_list(
        &mut self,
        u: UnitId,
        s: u16,
        owner: Option<(UnitType, u32)>,
        expire: i32,
    ) -> Option<ListId> {
        let (ty, guid) = match owner {
            Some(o) => o,
            None => {
                let r = self.units.get(u)?;
                (r.ty, r.guid)
            }
        };
        let l = self.stats.alloc(0, 0, ty.index() as u32, guid);
        self.stats.set_expire(l, expire);
        self.stats.set_state(l, u32::from(s));
        self.stats.attach(&mut *self.h, u, l, true);
        Some(l)
    }

    /// Hireling test `0x0063EE90`: a monster of a hireling class
    /// (`monsters/init.md` §6 step 4).
    pub fn is_hireling(&self, game: &Game, u: UnitId) -> bool {
        game.lists
            .unit(u)
            .is_some_and(|e| e.ty == UnitType::Monster)
            && self
                .units
                .get(u)
                .is_some_and(|r| HIRELING_CLASSES.contains(&r.class))
    }

    /// Unit allocation `0x00555230` (`units.md` §3.1) on the game seed:
    /// steps 1–7 with the per-kind init (an object's after the seed step
    /// is written back), then step 8: `SUNIT_Add`'s list part and its
    /// path part (`path-placement.md` §2.5, [`View::path_place`]; without
    /// the path provider [`Pending::place`]). An object allocated from
    /// inside an object call is left unlinked: its caller runs the init
    /// (`objects::allocate`) and then [`View::add_allocated`].
    pub fn allocate(
        &mut self,
        game: &mut Game,
        req: &AllocRequest,
        x: i32,
        y: i32,
    ) -> Option<UnitId> {
        let mut seed = self.h.game_seed;
        let outer = self.h.deferred_inits.replace(Vec::new());
        let r = {
            let mut sim = Sim {
                game,
                units: self.units,
                stats: self.stats,
                data: self.data,
            };
            crate::units::lifecycle::allocate_unlinked(&mut sim, &mut *self.h, &mut seed, req)
        };
        self.h.game_seed = seed;
        // The object init is the allocation's last step before `SUNIT_Add`
        // (`units.md` §3.1 r7; `quests-act1-rest.md` §9 item 7: after the
        // unit's seed step): run it now that the step is in the hooks.
        let inits = std::mem::replace(&mut self.h.deferred_inits, outer).unwrap_or_default();
        for u in inits {
            self.object_init(game, u);
        }
        match r {
            Ok(Some(u)) => {
                if req.ty == UnitType::Object && self.h.objects_out {
                    return Some(u);
                }
                self.add_allocated(game, u, req, x, y).then_some(u)
            }
            Ok(None) => None,
            Err(e) => {
                self.unit_error(e);
                None
            }
        }
    }

    /// Step 8 of an allocation (`units.md` §3.1): `SUNIT_Add` in the
    /// allocation's room, then the path part at (x, y). `false`: the add
    /// failed (logged).
    pub fn add_allocated(
        &mut self,
        game: &mut Game,
        u: UnitId,
        req: &AllocRequest,
        x: i32,
        y: i32,
    ) -> bool {
        let r = {
            let mut sim = Sim {
                game,
                units: self.units,
                stats: self.stats,
                data: self.data,
            };
            crate::units::lifecycle::add(&mut sim, &mut *self.h, u, req)
        };
        if let Err(e) = r {
            self.unit_error(e);
            return false;
        }
        self.path_place(game, u, x, y);
        true
    }

    /// The allocation room of a unit between steps 7 and 8 (`units.md`
    /// §3.1 r7.2), else the room it stands in.
    pub fn init_room(&self, game: &Game, u: UnitId) -> Option<crate::units::RoomId> {
        match self.h.alloc_rooms.iter().rev().find(|&&(v, _)| v == u) {
            Some(&(_, room)) => room,
            None => game.lists.unit(u).and_then(|e| e.room()),
        }
    }

    /// Unit removal `0x00555600` (`units.md` §3.2).
    pub fn remove(&mut self, game: &mut Game, u: UnitId) {
        let r = {
            let mut sim = Sim {
                game,
                units: self.units,
                stats: self.stats,
                data: self.data,
            };
            crate::units::lifecycle::remove(&mut sim, &mut *self.h, u)
        };
        if let Err(e) = r {
            self.unit_error(e);
        }
    }

    /// A monster mode change (`units.md` §4.6, `0x005A7C20`). Its umod
    /// callbacks run inside it ([`UnitHooks::monster_umods`]: mode 0
    /// before the start function, mode 1 after the animation prepare,
    /// `umod-callbacks.md` §2) on the lent monster world.
    pub fn monster_set_mode(&mut self, game: &mut Game, u: UnitId, mode: u32) -> bool {
        let mut sim = Sim {
            game,
            units: self.units,
            stats: self.stats,
            data: self.data,
        };
        let r = crate::units::modes::monster_set_mode(&mut sim, &mut *self.h, u, mode);
        match r {
            Ok(()) => true,
            Err(e) => {
                self.unit_error(e);
                false
            }
        }
    }
}

/// Clears state 54 and, for players, state 92 (`0x005544B0` minus the
/// timer part, `ai.md` §1.1).
pub fn clear_uninterruptable<X: Pending>(v: &mut View<'_, X>, game: &Game, u: UnitId) {
    v.set_state(u, state::UNINTERRUPTABLE as u16, false);
    if game.lists.unit(u).is_some_and(|e| e.ty == UnitType::Player) {
        v.set_state(u, STATE_DEATH_DELAY, false);
    }
}
