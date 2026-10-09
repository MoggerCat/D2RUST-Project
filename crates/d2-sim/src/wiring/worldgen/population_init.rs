// Spec: specs/monsters/population.md §6, §9.6, §10, §11.4, §11.5 r4 (the MonsterInit seam); specs/sim/unit-events.tsv (0x0054ea84); specs/monsters/init.md §4, §5, §14, §16–§20; specs/sim/units.md §3.1
//! Population → monster init and unit allocation: [`MonsterInit`] on
//! [`WorldHost`]. The creation call of `population.md` §9.6 is the one
//! creation path: allocation `0x00555230` (`units.md` §3.1, one game-seed
//! step) followed by the monster type init `0x00574250`
//! ([`init::type_init`]); per-class extras `0x005B21B0` are init's normal
//! mods, `0x005B1CF0` its boss mods; the boss calls go to init's umod
//! choice, transfer and init functions. Monster data lives in
//! [`super::WorldState::monsters`].
//!
//! The type init runs right after the allocator returns: the allocator's
//! per-kind hook (`LifecycleHooks::init_kind`) is its last step, so
//! nothing the allocator does follows it.

use crate::monsters::ai::MapNode;
use crate::monsters::init;
use crate::monsters::population::{Alloc, CoordRect, MonsterInit, OwnerKey, PopState, PresetUnit};
use crate::units::lifecycle::AllocRequest;
use crate::units::{RoomId, UnitId, UnitType};

use super::{WorldHost, WorldPending, WorldgenError};

/// `population.md` §13 / `init.md` §4 step 2: monster flag 2 (+0x5C) is
/// the "not counted" flag.
const FLAG_NOT_COUNTED: u32 = 2;
/// Champion pack member `0x005A48C0` (`init.md` §16.2).
const UMOD_CHAMPION: u8 = 16;
/// Quest modifier `0x005A4850(…, 22, 1)` (`init.md` §20 step 5).
const UMOD_QUEST: u8 = 22;

impl<X: WorldPending> MonsterInit for WorldHost<'_, X> {
    /// `0x00555230(type 1, …)` then the type init `0x00574250` with the
    /// game's regions in place (`init.md` §5 step 4 → `population.md`
    /// §2.5).
    fn allocate_monster(&mut self, a: Alloc, state: &mut PopState) -> Option<UnitId> {
        let class = u32::try_from(a.class).ok()?;
        let req = AllocRequest {
            ty: UnitType::Monster,
            class,
            room: Some(a.room),
            add: true,
            fixed_guid: a.guid,
            mode: u32::from(a.mode),
            allied: false,
        };
        // `0x00555230`: the type init `0x00574250` runs inside the
        // allocator, before `SUNIT_Add` (`init.md` §4.1), whose monster
        // branch schedules the first think.
        let unit = self.v.allocate_unadded(self.game, &req)?;
        self.with_state(state, |h| {
            h.init(|cx, h| init::type_init(cx, h, unit));
        });
        self.v
            .add_allocated(self.game, unit, &req, a.x, a.y)
            .then_some(unit)
    }

    fn set_monster_flag(&mut self, unit: UnitId, flag: u32) {
        if flag == FLAG_NOT_COUNTED {
            self.w.monsters.entry(unit).not_counted = true;
        } else {
            self.v.h.x.set_monster_flag(unit, flag);
        }
    }

    fn set_coord_record(
        &mut self,
        unit: UnitId,
        rect: Option<CoordRect>,
        room: RoomId,
        x: i32,
        y: i32,
    ) {
        self.v.h.x.set_coord_record(unit, rect, room, x, y);
    }

    /// `0x005543B0`: the unit's state-105 list
    /// ([`crate::wiring::action::View::set_alignment`]),
    /// then the host's copy.
    fn set_alignment(&mut self, unit: UnitId, align: u8) {
        self.v.set_alignment(self.game, unit, align);
        self.v.h.x.set_alignment(unit, align);
    }

    /// Unit flags (+0xC4) `|=`.
    fn set_unit_flags(&mut self, unit: UnitId, flags: u32) {
        if let Some(r) = self.v.units.get_mut(unit) {
            r.flags |= flags;
        }
    }

    /// `0x005B21B0`: normal mods (`init.md` §14.1).
    fn class_extras(&mut self, unit: UnitId) {
        self.init(|cx, h| init::normal_mods(cx, h, unit));
    }

    /// `0x005B1CF0`: boss mods (`init.md` §14.2).
    fn init_monster(&mut self, unit: UnitId) {
        self.init(|cx, h| init::boss_mods(cx, h, unit));
    }

    fn unit_class(&self, unit: UnitId) -> i32 {
        self.v.units.get(unit).map_or(-1, |r| r.class as i32)
    }

    /// `0x00573520`: monster data +0x58.
    fn unit_level(&self, unit: UnitId) -> i32 {
        self.w.monsters.get(unit).map_or(0, |m| m.level_id)
    }

    fn type_flags(&self, unit: UnitId) -> u16 {
        self.w.monsters.get(unit).map_or(0, |m| m.type_flags)
    }

    fn set_type_flags(&mut self, unit: UnitId, flags: u16) {
        self.w.monsters.entry(unit).type_flags |= flags;
    }

    /// `0x005A0760` (`init.md` §17).
    fn boss_modifiers(&mut self, boss: UnitId, champion_allowed: bool) {
        self.init(|cx, h| init::choose_umods(cx, h, boss, champion_allowed));
    }

    /// The tail of `0x005A2120`: `init.md` §18 step 2 (population spawned
    /// and linked the minions, `population.md` §6.5).
    fn boss_modifier_init(&mut self, boss: UnitId) {
        self.init(|cx, h| init::boss_minions_and_init(cx, h, boss, 0, 0, None, false));
    }

    /// `0x005A48C0` (16, `init.md` §16.2) or `0x005A4850(…, 22, 1)`; a
    /// superunique runs its `init.md` §20 steps 4–5 right before its
    /// closing 22.
    fn add_modifier(&mut self, unit: UnitId, m: u8, state: &mut PopState) {
        self.with_state(state, |h| match m {
            UMOD_CHAMPION => h.init(|cx, h| init::champion_pack_member(cx, h, unit, m)),
            UMOD_QUEST => {
                if let Some((row, aura)) = h.w.superunique_tail.remove(&unit) {
                    h.init(|cx, h| init::superunique_finish(cx, h, unit, row, aura));
                }
                h.init(|cx, h| init::assign_umod(cx, h, unit, m, true));
            }
            _ => h.w.errors.push(WorldgenError::Modifier(m)),
        });
    }

    /// `0x005A0930` (`init.md` §18 step 1).
    fn transfer_modifiers(&mut self, boss: UnitId, minion: UnitId) {
        self.init(|cx, h| init::xfer_umods(cx, h, boss, minion));
    }

    /// `0x0058F030(game, unit, GUID, type a, f1 b, f2 c)`: the AI control
    /// record's minion owner (+0x2C GUID, +0x30 type;
    /// `umod-callbacks.md` §1 rule 5), read back by `0x0058F0D0` (the pack
    /// leader test of the Fallen body, `ai-bodies.md` §9.4 step 5.2).
    /// Both keys name the unit's GUID (`0x00451F50`: unit +0x0C,
    /// `population.md` §10.2 step 3).
    ///
    /// PROVISIONAL (umod-callbacks.md §1 r5, REC-892): what the restart
    /// `0x005DD230` does for f1 / f2 ≠ 0 is not stated; nothing is done
    /// for them here.
    fn set_owner_data(&mut self, unit: UnitId, owner: OwnerKey, a: i32, b: i32, c: i32) {
        let (OwnerKey::Guid(o) | OwnerKey::DataOf(o)) = owner;
        let guid = self.game.lists.unit(o).map(|e| e.guid);
        let ty = u8::try_from(a)
            .ok()
            .and_then(|t| UnitType::ALL.get(usize::from(t)).copied());
        if let (Some(guid), Some(ty)) = (guid, ty) {
            if let Some(ctl) = self.v.h.ai.as_mut().and_then(|s| s.control_mut(unit)) {
                ctl.minion_owner = Some(crate::monsters::ai::UnitRef { ty, guid });
            }
        }
        self.v.h.x.set_owner_data(unit, owner, a, b, c);
    }

    /// `0x0058F100`: also the leader's AI control minion list (+0x34,
    /// GUIDs), which the command copy `0x0058F730` walks (`ai.md` §8).
    fn add_minion(&mut self, leader: UnitId, minion: UnitId) {
        self.w.minions.entry(leader).or_default().push(minion);
        if let Some(guid) = self.game.lists.unit(minion).map(|e| e.guid) {
            if let Some(ctl) = self.v.h.ai.as_mut().and_then(|s| s.control_mut(leader)) {
                ctl.minions.push(guid);
            }
        }
    }

    /// `0x005DD330`.
    fn set_owner(&mut self, minion: UnitId, owner: UnitId) {
        self.w.owners.insert(minion, owner);
    }

    fn boss_quest_hook(&mut self, boss: UnitId) {
        self.v.h.x.boss_quest_hook(boss);
    }

    /// `init.md` §20 steps 1–2; steps 4–5 wait for the closing modifier
    /// 22 (`population.md` §11.4 runs the minions, step 3, in between).
    // TODO(population.md §11.4 step 6, init.md §20 steps 4–5): the order
    // of the hcIdx extra spawns and the aura re-run / quest records is
    // not stated; the spawns run first here.
    fn superunique_init(&mut self, boss: UnitId, su: i32) {
        let Ok(row) = u16::try_from(su) else {
            return;
        };
        if let Some(aura) = self.init(|cx, h| init::superunique_mods(cx, h, boss, row)) {
            self.w.superunique_tail.insert(boss, (row, aura));
        }
    }

    /// `0x00555230(type 2, class, …)` with the object init
    /// (`objects.md` §3, §6) on the action wiring's object state
    /// ([`crate::wiring::action::View::create_object`]); a game without
    /// one: [`WorldPending::create_object`].
    ///
    /// Allocated in mode 0 with flag 1 and GUID 0 (`objects-2.md` §22
    /// rule 4: evilhut and barricade-door objects).
    fn create_object(&mut self, room: RoomId, class: i32, x: i32, y: i32) {
        match u32::try_from(class) {
            Ok(c) if self.v.h.objects.is_some() => {
                self.v.create_object(self.game, room, c, x, y, 0);
            }
            _ => self.v.h.x.create_object(room, class, x, y),
        }
    }

    /// `monsters/ai.md` open question 8: the preset path's points (level
    /// sub-tiles, `drlg/preset.md` §7, §9) become the map-AI nodes
    /// (action, x, y) of the monster's AI control. A monster without an
    /// AI control gets none; the preset's path is cleared either way.
    fn move_preset_path(&mut self, unit: UnitId, room: RoomId, index: usize) {
        let Some((act, r)) = self.drlg_room(room) else {
            return;
        };
        let path = self
            .w
            .types
            .borrow_mut()
            .act_presets_mut(act)
            .and_then(|p| p.take_unit_path(r, index));
        let (Some(path), Some(ai)) = (path, self.v.h.ai.as_mut()) else {
            return;
        };
        if let Some(c) = ai.control_mut(unit) {
            c.map_ai = Some(
                path.iter()
                    .map(|p| MapNode {
                        action: p.action as i32,
                        x: p.x,
                        y: p.y,
                    })
                    .collect(),
            );
        }
    }

    fn preset_created(&mut self, unit: UnitId, preset: &PresetUnit) {
        self.v.h.x.preset_created(unit, preset);
    }

    /// `population.md` §11.5 rule 4, class 438: event 7 (MONUMOD) at
    /// frame + 250 + `roll(50)` on the created monster's own seed
    /// (`unit-events.tsv` site `0x0054ea84`).
    fn schedule_monumod(&mut self, unit: UnitId) {
        let r = self
            .v
            .units
            .get_mut(unit)
            .map_or(0, |rec| rec.seed.roll(50)) as i32;
        let at = self.game.frame.wrapping_add(250).wrapping_add(r);
        // A unit outside the lists schedules nothing.
        let _ = self
            .game
            .schedule_event(unit, init::EVENT_UMOD, at, None, 0, 0);
    }

    fn change_alignment(&mut self, unit: UnitId, a: i32, b: i32) {
        self.v.h.x.change_alignment(unit, a, b);
    }

    fn restore_inactive_units(&mut self, room: RoomId) {
        self.v.h.x.restore_inactive_units(room);
    }

    /// `0x00552610` (`object-population.md`) on the action wiring's
    /// object state with the room facts of the DRLG; a game without one:
    /// [`WorldPending::populate_objects`].
    fn populate_objects(&mut self, room: RoomId) {
        if self.v.h.objects.is_none() {
            self.v.h.x.populate_objects(room);
            return;
        }
        let info = self.object_room_info(room);
        self.v.populate_objects(self.game, &info);
    }
}
