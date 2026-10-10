// Spec: specs/sim/intents-events.md §7.3, §7.4, §7.5, §7.7, §7.9 rules 2–3; specs/sim/pathing.md §9.8; specs/sim/units.md §4.1, §4.6; specs/audio/triggers-2.md §14
//! The monster part of the per-unit update (`0x00598220`, §7.3 rule 2
//! step 2: the mode message `0x00597E20`, S→C 0x67–0x6D, built by
//! [`crate::monsters::mode_message`]), the flag part of the room
//! clean-up `0x00553220` (§7.5 steps 3 and 7), and the two death-mode
//! event functions that end a monster's death in mode 12 (§7.7 rule 3:
//! event 0 `0x005A7350`, event 1 `0x005A72B0`).
//!
//! Who reads what: the mode message runs in the client pass (`tick.md`
//! §6 step 5, [`View::monster_update`] from `ActionSim::send_unit_update`)
//! for each queued monster with unit flag 0x1; the clean-up runs on every
//! unit of every update queue after all clients (`tick.md` §3 step 6,
//! [`View::room_cleanup`]), so each mode set is sent once per client.

use crate::game::Game;
use crate::monsters::mode_message::{self, ModeInput, ModeMessage, MODE_ROWS};
use crate::path::record::flags as path_flags;
use crate::path::UnitPath;
use crate::units::hooks::Sim;
use crate::units::lists::ClientId;
use crate::units::modes::{self, monster_mode};
use crate::units::record::{flags, Anim};
use crate::units::{UnitId, UnitType};

use super::{ActionHooks, Pending, View, WiringError};

/// Stat 67 `velocitypercent` (§7.7 rule 4).
const STAT_VELOCITY: u16 = 67;
/// Stat 29 `lastexp` (§7.5 step 7).
const STAT_LASTEXP: u16 = 29;
/// Stat 328, the position stat (§7.4 rule 5, mode 1).
const STAT_POSITION: u16 = 328;
/// Stat 6 `hitpoints`.
const STAT_LIFE: u16 = 6;
/// §7.3 rule 2 step 1: flag-ex (+0xC8) bit 0x10000, the reassign
/// request (cleared by the room clean-up, §7.5 step 3).
const REASSIGN_EX: u32 = 0x1_0000;

/// §7.3 rule 2 step 7: unit flag 0x8000, the monster was hit.
pub const HIT: u32 = 0x8000;

/// Unit flags (+0xC4) and flag-ex bits (+0xC8) of the room clean-up.
pub mod cleanup {
    /// §7.5 step 3: unit flags 0x1, 0x10, 0x400, 0x8000.
    pub const UNIT_FLAGS: u32 = 0x1 | 0x10 | 0x400 | 0x8000;
    /// §7.5 step 3: flag-ex 0x800, 0x1000, 0x10000, 0x200000.
    pub const FLAGS_EX: u32 = 0x800 | 0x1000 | 0x10000 | 0x200000;
    /// §7.5 step 7: unit flag 0x100 (players, monsters, objects).
    pub const FLAG_100: u32 = 0x100;
    /// §7.5 step 7: a monster's unit flag 0x800.
    pub const MONSTER_800: u32 = 0x800;
    /// §7.5 step 7: a monster's flag-ex 0x10000.
    pub const MONSTER_EX: u32 = 0x10000;
    /// §7.5 step 7: an item's unit flag 0x1000.
    pub const ITEM_1000: u32 = 0x1000;
}

/// Monster mode functions of mode DT (`units.md` §4.6 table).
pub const DT_EVENT0: u32 = 0x005A_7350;
pub const DT_EVENT1: u32 = 0x005A_72B0;

/// The `monstats` base id (row +0x02, `0x0063E8D0(unit, 0)`) whose death
/// walks until its animation ends (§7.7 rule 3).
const STEPPING_DEATH_BASE: u16 = 78;

/// Fatal assertions and gaps of the mode message.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModeMessageError {
    /// §7.4 rule 4: the unit has no (dynamic) path (fatal 0xE6).
    NoPath(UnitId),
}

impl<X: Pending> View<'_, X> {
    /// Unit +0xB0 (`combat/damage.md` §7.1 step 2), as the messages send
    /// it (u8): e of a monster mode-0 / mode-3 message, f of a mode-13
    /// one (§7.4 rule 5), b of 0x0C / 0x0D.
    pub fn unit_b0(&self, unit: UnitId) -> u8 {
        self.units.get(unit).map_or(0, |r| r.hit_class as u8)
    }

    /// The monster update `0x00598220` (§7.3 rule 2) for the client
    /// `client`, its messages sent to the client's player
    /// ([`Pending::send`]); a client without a player gets nothing. Needs
    /// the path provider (the caller checks it). In order:
    ///
    /// 1. flag-ex (+0xC8) bit 0x10000: S→C 0x15 (type 1, GUID, the path's
    ///    cell, flag 1), then the room-change messages `0x00554670(game,
    ///    unit, 0)` (`pathing.md` §9.8);
    /// 2. unit flag 0x1: the mode message (§7.4), then unit flag 0x80000
    ///    := 0;
    /// 4. unit flag 0x100: the overhead message `0x00571620` (§7.9 rule 3);
    /// 6. unit flag 0x400: S→C 0x2C (`0x00571740`, `audio/triggers-2.md`
    ///    §14 rule 2, [`crate::units::sound::sound_message`]);
    /// 7. unit flag 0x8000 (hit): S→C 0x0C (`0x00597CF0`,
    ///    [`skill_message::monster_hit`]);
    /// 9. the unit's list has the overlay flag (`0x00625A20`): S→C 0x11
    ///    (`0x0053D850`, `units::messages::report_kill`) with the overlay
    ///    list's stat 178 when it is in 0..=[`super::ActionTables::overlay_count`];
    /// 10. unit flag 0x800 and monster data +0x5C bit 1 (`0x00573540(unit,
    ///     1)`): S→C 0x57 (`0x00597C70` → `0x0053D880`,
    ///     `units::messages::npc_enchants`).
    ///
    /// First (§7.1 rule 2.1): a monster with unit flag 0x10 (not yet
    /// announced) gets its add messages (§7.2, [`View::monster_add`]:
    /// 0xAC, 0x98, 0x21, 0xAA, part B with a mode message). Of rule 2,
    /// step 3 sends the pending event records
    /// ([`View::send_event_records`], §7.9 rule 2) and steps 5
    /// and 8 are not sent: step 5 needs the item world, step 8's test
    /// `0x00639F20` and stat sender `0x005711D0` have no d2rs provider
    /// yet.
    pub fn monster_update(&mut self, game: &mut Game, client: ClientId, unit: UnitId) {
        let Some(receiver) = game.lists.client(client).and_then(|c| c.player) else {
            return;
        };
        let Some(r) = self.units.get(unit) else {
            return;
        };
        if r.ty != UnitType::Monster {
            return;
        }
        let (unit_flags, flags_ex, guid) = (r.flags, r.flags2, r.guid);
        // §7.1 rule 2.1 (announced := 1 feeds only step 8, not sent).
        if unit_flags & flags::SEED_SET != 0 && unit != receiver {
            self.monster_add(game, receiver, unit);
        }
        // Step 1.
        if flags_ex & REASSIGN_EX != 0 {
            let (x, y) = self.h.path_position(unit);
            let m = crate::path::walk::messages::reassign_player(
                UnitType::Monster as u8,
                guid,
                x as u16,
                y as u16,
                1,
            );
            self.h.x.send(receiver, &m);
            crate::wiring::path::walk::PathCtx::of(self, game).room_change_messages(unit);
        }
        // Step 2.
        if unit_flags & flags::CHANGED != 0 {
            self.mode_update(game, client, receiver, unit);
        }
        // Step 3: the pending event records (`0x00571CD0`, §7.9 rule 2).
        self.send_event_records(game, receiver, unit);
        // Step 4.
        if unit_flags & flags::HOVER_FREED != 0 {
            self.overhead_message(receiver, unit, UnitType::Monster as u8, guid);
        }
        // Step 6: unit flag 0x400 → `0x00571740` (`audio/triggers-2.md`
        // §14 rule 2).
        if let Some(m) = crate::units::sound::sound_message(game, unit, receiver) {
            self.h.x.send(receiver, &m);
        }
        // Step 7.
        if unit_flags & HIT != 0 {
            self.hit_message(receiver, unit, guid);
        }
        // Step 9: the unit's list has the overlay flag (`0x00625A20`) →
        // `0x005715A0`: stat 178 of the overlay list, 0..=overlay count
        // (inclusive) → S→C 0x11 (`0x0053D850`).
        if let Some(v) = self.stats.overlay_to_send(unit) {
            if (0..=self.h.tables.overlay_count).contains(&v) {
                let m =
                    crate::units::messages::report_kill(UnitType::Monster as u8, guid, v as u16);
                self.h.x.send(receiver, &m);
            }
        }
        // Step 10: unit flag 0x800 → `0x00597C70`: monster data +0x5C
        // bit 1 (`0x00573540(unit, 1)`) → S→C 0x57 (`0x0053D880`).
        if unit_flags & cleanup::MONSTER_800 != 0 {
            if let Some(d) = self.h.monster_data(unit).filter(|d| d.data_flag1) {
                let m = crate::units::messages::npc_enchants(
                    guid,
                    d.name_seed,
                    [d.umods[0], d.umods[1], d.umods[2]],
                    d.has_flag(4),
                );
                self.h.x.send(receiver, &m);
            }
        }
    }

    /// §7.3 rule 2 step 2: the mode message (§7.4), then unit flag
    /// 0x80000 := 0.
    fn mode_update(&mut self, game: &Game, client: ClientId, receiver: UnitId, unit: UnitId) {
        self.mode_message(game, client, receiver, unit);
        if let Some(r) = self.units.get_mut(unit) {
            r.flags &= !flags::MODE_CHANGING;
        }
    }

    /// The mode message `0x00597E20` (§7.4) of `unit` to `receiver`, the
    /// player of `client`.
    pub(super) fn mode_message(
        &mut self,
        game: &Game,
        client: ClientId,
        receiver: UnitId,
        unit: UnitId,
    ) {
        if let Some(input) = self.mode_input(game, client, unit) {
            match mode_message::mode_message(&input) {
                ModeMessage::Send(b) => self.h.x.send(receiver, &b),
                ModeMessage::Stop(b) => {
                    self.h.x.send(receiver, &b);
                    let v = self.stats.unit_base(unit, STAT_POSITION, 0);
                    self.stats
                        .unit_set(&mut *self.h, unit, STAT_POSITION, v.wrapping_add(1), 0);
                }
                ModeMessage::Skill { .. } => {
                    self.skill_message(
                        game,
                        receiver,
                        unit,
                        (UnitType::Monster as u8, input.guid),
                        input.target,
                        input.path_target,
                    );
                }
                ModeMessage::Nothing => {}
            }
        }
    }

    /// The reads of `0x00597E20` (§7.4 rules 2–5, §7.7 rule 4). `None`:
    /// no dynamic path (rule 4's fatal, logged).
    fn mode_input(&mut self, game: &Game, client: ClientId, unit: UnitId) -> Option<ModeInput> {
        let r = self.units.get(unit)?;
        let (mode, guid) = (r.mode, r.guid);
        let row = MODE_ROWS.get(mode as usize).copied();
        let target = self.mode_target(game, client, unit);
        let target = target.filter(|_| row.is_some_and(|e| e.use_target));
        let Some(path) = self.h.paths.as_ref().and_then(|p| p.dynamic(unit)) else {
            self.h
                .errors
                .push(WiringError::ModeMessage(ModeMessageError::NoPath(unit)));
            return None;
        };
        let life = crate::stats::life_fraction(
            self.stats.unit_total(unit, STAT_LIFE, 0),
            crate::combat::vitals::VitalsUnits::max_life(self, unit),
        );
        Some(ModeInput {
            mode,
            guid,
            skill_in_use: self.h.used_skill_of(unit).is_some(),
            target,
            cell: (path.x() as u16, path.y() as u16),
            path_target: (path.target_x, path.target_y),
            direction: path.direction,
            path_type: path.path_type,
            path_90: path.dist_budget,
            max_distance: path.max_distance,
            stop_distance: path.stop_distance,
            unit_b0: self.unit_b0(unit),
            life: life as u8,
            flag_100: self.h.x.monster_flag_100(unit),
            velocity: self.stats.unit_total(unit, STAT_VELOCITY, 0),
        })
    }

    /// T of §7.4 rule 2: the unit's target `0x00553540` (`skills/bodies.md`
    /// §2.1: the refresh `0x00553490` clears a stale or picked-up path
    /// target; the unit itself is none), kept only when its room's client
    /// array holds `client` (`0x005387F0`, `drlg/rooms.md` §7 rule 1).
    /// Returns (type, GUID).
    fn mode_target(&mut self, game: &Game, client: ClientId, unit: UnitId) -> Option<(u8, u32)> {
        let tu = self.h.paths.as_ref()?.dynamic(unit)?.target_unit?;
        let found = game.lists.find_unit(tu.ty, tu.guid);
        let stale = match found {
            Some(f) if f == tu.unit => self
                .units
                .get(f)
                .is_some_and(|r| r.ty == UnitType::Item && matches!(r.mode, 1 | 2)),
            _ => true,
        };
        if stale {
            if let Some(d) = self.h.paths.as_mut().and_then(|p| p.dynamic_mut(unit)) {
                d.target_unit = None;
            }
            return None;
        }
        if tu.unit == unit {
            return None;
        }
        let room = game.lists.unit(tu.unit)?.room()?;
        let (drlg, id) = self.h.drlg.drlg_room(game, room)?;
        drlg.active_room(id)?
            .clients
            .contains(&client)
            .then_some((tu.ty as u8, tu.guid))
    }

    /// The room clean-up `0x00553220(game, unit)` (§7.5), in order:
    /// 1. the changed-stat (mod) array emptied (`0x00625960`,
    ///    `stat-lists.md` §11.3; only an extended list has one);
    /// 2. the pending event records freed (§7.9 rule 2);
    /// 3. unit flags 0x1, 0x10, 0x400, 0x8000; the path's room-changed
    ///    flag (`0x00620FA0(unit, 0)`); flag-ex 0x800, 0x1000, 0x10000,
    ///    0x200000;
    /// 4. the state-changed bits zeroed (`0x00639EE0`);
    /// 5. the update-list reset `0x00597B00`: run by the server's item
    ///    update pass after the tick (`d2-server`
    ///    `handlers::items::moves::update_pass`), which also reads and
    ///    clears the item flags 0x20 / 0x2000 of step 7;
    /// 6. twice: the list has 0x100 (`0x00625A20`) → overlay removal
    ///    `0x00627410` (`stat-lists.md` §8.8.2);
    /// 7. player: stat 29 `lastexp` := −1, unit flag 0x100 := 0 (the
    ///    client record's +0x34 → +4 := 0 is [`Pending::client_cleanup`]);
    ///    monster: flag 0x100 := 0, flag 0x800 set → cleared and monster
    ///    data +0x5C bit 0x1 := 0 (`0x00573570`), flag-ex 0x10000 := 0;
    ///    object: flag 0x100 := 0; item: unit flag 0x1000 := 0.
    pub fn room_cleanup(&mut self, unit: UnitId) {
        // Step 1.
        self.stats.clear_mods(unit);
        // Step 2.
        self.h.event_records.clear(unit);
        let Some(r) = self.units.get_mut(unit) else {
            return;
        };
        // Step 3.
        let flags_before = r.flags;
        r.flags &= !cleanup::UNIT_FLAGS;
        r.flags2 &= !cleanup::FLAGS_EX;
        let ty = r.ty;
        // Step 7 (flags).
        match ty {
            UnitType::Player | UnitType::Object => r.flags &= !cleanup::FLAG_100,
            UnitType::Monster => {
                r.flags &= !(cleanup::FLAG_100 | cleanup::MONSTER_800);
                r.flags2 &= !cleanup::MONSTER_EX;
            }
            UnitType::Item => r.flags &= !cleanup::ITEM_1000,
            UnitType::Missile | UnitType::Tile => {}
        }
        match self.h.paths.as_mut().and_then(|p| p.records.get_mut(&unit)) {
            Some(UnitPath::Dynamic(d)) => d.flags &= !path_flags::ROOM_CHANGED,
            Some(UnitPath::Static(s)) => s.room_changed = 0,
            None => {}
        }
        // Step 4.
        self.stats.clear_states_changed(unit);
        // Step 6.
        for _ in 0..2 {
            let flagged = self.stats.unit_list(unit).is_some_and(|l| {
                self.stats.flags(l) & crate::stats::lists::flag::REMOVE_OVERLAY != 0
            });
            if flagged {
                self.stats.remove_overlay(&mut *self.h, unit);
            }
        }
        // Step 7 (the rest).
        match ty {
            UnitType::Player => {
                self.stats.unit_set(&mut *self.h, unit, STAT_LASTEXP, -1, 0);
                self.h.x.client_cleanup(unit);
            }
            UnitType::Monster if flags_before & cleanup::MONSTER_800 != 0 => {
                if let Some(m) = self
                    .h
                    .monster_world
                    .as_mut()
                    .and_then(|w| w.monster_mut(unit))
                {
                    m.data_flag1 = false;
                }
            }
            _ => {}
        }
    }
}

/// `0x006217C0`: the animation is complete (§7.7 rule 3): no sequence →
/// current frame (+0x44) + speed (+0x4C) ≥ frame count (+0x48); with a
/// sequence → frame count ≤ 0.
pub fn anim_complete(a: &Anim) -> bool {
    match a.sequence {
        None => a.frame + i32::from(a.speed) >= a.frame_count,
        Some(_) => a.frame_count <= 0,
    }
}

impl<X: Pending> ActionHooks<X> {
    /// The frame advance `0x00623E00` (`sim/units.md` §4.2): the
    /// sequence branch when the unit has a sequence, else the frame
    /// advance with the frame bonus `0x00623B10`.
    pub fn refresh_unit_animation(&mut self, sim: &mut Sim<'_>, unit: UnitId) {
        let bonus = self.frame_bonus_in(sim.units, sim.stats, unit);
        if let Some(r) = sim.units.get_mut(unit) {
            if !crate::units::anim::advance_sequence(&mut r.anim) {
                crate::units::anim::advance_frame(&mut r.anim, bonus);
            }
        }
    }

    /// `0x006510C0(class, 0, 0)`: the monstats chain position (+0x4B, set
    /// by the data fix-up `specs/data/fixups.md` §8): the number of
    /// `NextInClass` steps from the class's `BaseId` to the class; 0 when
    /// the walk does not reach it.
    pub(crate) fn monster_chain_position(&self, class: i32) -> i32 {
        let rows = &self.tables.combat.monstats;
        let Some(mut cur) = usize::try_from(class)
            .ok()
            .and_then(|c| rows.get(c))
            .map(|m| usize::from(m.baseid))
        else {
            return 0;
        };
        for steps in 0..=255 {
            if cur as i32 == class {
                return steps;
            }
            let Some(next) = rows.get(cur).map(|m| usize::from(m.nextinclass)) else {
                break;
            };
            if next == cur || next >= rows.len() {
                break;
            }
            cur = next;
        }
        0
    }

    /// Mode DT's event-0 function `0x005A7350` (§7.7 rule 3): a monster
    /// of `monstats` base id 78 takes a path step (`0x00554CA0`),
    /// refreshes its animation (`0x00623E00`, [`Pending::refresh_animation`])
    /// and sets mode 12 only once its animation is complete
    /// ([`anim_complete`]); every other monster sets mode 12 at once.
    pub fn death_event0(&mut self, sim: &mut Sim<'_>, unit: UnitId) {
        let class = sim.units.get(unit).map(|r| r.class);
        let base = class.and_then(|c| {
            self.tables
                .combat
                .monstats
                .get(c as usize)
                .map(|m| m.baseid)
        });
        if base == Some(STEPPING_DEATH_BASE) {
            {
                let mut v = View::of(sim.units, sim.stats, sim.data, self);
                if v.h.paths.is_some() {
                    crate::wiring::path::walk::unit_step(&mut v, sim.game, unit);
                } else {
                    v.h.x.step(sim.game, unit);
                }
            }
            self.refresh_unit_animation(sim, unit);
            if !sim.units.get(unit).is_some_and(|r| anim_complete(&r.anim)) {
                return;
            }
        }
        self.death_mode(sim, unit);
    }

    /// Mode DT's event-1 function `0x005A72B0` (§7.7 rule 3): mode 12,
    /// unit event 13 (`0x005C0C30(game, 13, unit, 0, 0)`,
    /// [`super::combat::CombatView::fire_unit_event`]), then the `monstats` `SplEndDeath` (row
    /// +0x1A4) action 1 or 2 ([`Pending::death_end_action`] with
    /// `minion1`, row +0x26).
    pub fn death_event1(&mut self, sim: &mut Sim<'_>, unit: UnitId) {
        self.death_mode(sim, unit);
        View::of(sim.units, sim.stats, sim.data, self)
            .combat(sim.game)
            .fire_unit_event(13, Some(unit), None, None);
        let class = sim.units.get(unit).map(|r| r.class);
        let row = class.and_then(|c| self.tables.combat.monstats.get(c as usize));
        if let Some((action, minion)) = row.map(|m| (m.splenddeath, m.minion1)) {
            if matches!(action, 1 | 2) {
                self.x.death_end_action(sim.game, unit, action, minion);
            }
        }
    }

    /// "Sets mode 12" of §7.7 rule 3 as the mode set `0x00553570`
    /// (`units.md` §4.1): mode, unit flag 0x1, the update queue.
    ///
    /// Mode 12 is always set by the plain mode set `0x00553570`
    /// (`sim/units.md` §4.6 "Death and dead functions"), never by
    /// `0x005A7C20`.
    fn death_mode(&mut self, sim: &mut Sim<'_>, unit: UnitId) {
        if let Err(e) = modes::set_mode(sim, self, unit, monster_mode::DD) {
            self.errors.push(WiringError::Unit(e));
        }
    }
}

/// [`crate::units::hooks::UnitHooks::monster_mode_function`] for the two DT event functions:
/// `true` when `address` is one of them (and it ran).
pub fn death_function<X: Pending>(
    h: &mut ActionHooks<X>,
    sim: &mut Sim<'_>,
    unit: UnitId,
    address: u32,
) -> bool {
    match address {
        DT_EVENT0 => h.death_event0(sim, unit),
        DT_EVENT1 => h.death_event1(sim, unit),
        _ => return false,
    }
    true
}

pub mod skill_message;

#[cfg(test)]
mod tests;
