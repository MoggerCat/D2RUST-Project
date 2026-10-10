// Spec: specs/missiles/missiles.md (Related specs; the seams to other systems)
//! What missile code needs from systems owned by other specs. Each trait
//! names its expected provider; tests use small fakes. Nothing here
//! decides missile behaviour: the rules stay in the missile modules.

use crate::game::Game;
use crate::rng::Seed;
use crate::units::{RoomId, UnitId};

use super::hit::Damage;
use super::MissileData;

/// Units, stats and states. Provider: the units/stats session
/// (`sim/units.md`, `sim/stats.md`, `sim/stat-lists.md`).
pub trait MissileUnits {
    /// Unit allocation `0x00555230` for a missile: one game-seed step
    /// derives the unit seed (`rng.md` §5.3), the type-3 GUID counter
    /// advances, the unit enters `room` at (x, y) with `mode`. Missile
    /// init ([`super::init_missile`]) runs right after. `None` = failure.
    fn alloc_missile(
        &mut self,
        game: &mut Game,
        class: u16,
        x: i32,
        y: i32,
        room: RoomId,
        mode: u8,
    ) -> Option<UnitId>;
    /// Unit removal `0x00555600` (frees the unit, cancels its timers).
    fn remove_unit(&mut self, game: &mut Game, unit: UnitId);
    /// The unit seed (unit +0x20).
    fn seed(&mut self, unit: UnitId) -> &mut Seed;
    /// Position in subtiles.
    fn position(&self, unit: UnitId) -> (i32, i32);
    /// Unit size (`0x00620510`).
    fn size(&self, unit: UnitId) -> i32;
    /// Stat value (`0x00625480`-style read, layer 0).
    fn stat(&self, unit: UnitId, stat: u16) -> i32;
    /// Base stat value (`0x006253B0`).
    fn base_stat(&self, unit: UnitId, stat: u16) -> i32;
    fn set_stat(&mut self, unit: UnitId, stat: u16, value: i32);
    fn has_state(&self, unit: UnitId, state: u16) -> bool;
    /// `stat` of the stat list of `state` on the unit; `None` without one.
    fn state_stat(&self, unit: UnitId, state: u16, stat: u16) -> Option<i32>;
    /// Allocates the missile's stat list (`0x00626D40`).
    fn alloc_stat_list(&mut self, unit: UnitId);
    /// Unit flag test / set (unit +0xC4).
    fn unit_flag(&self, unit: UnitId, bit: u32) -> bool;
    fn set_unit_flag(&mut self, unit: UnitId, bit: u32, on: bool);
    /// Hostility test `0x00554200` (D2MOO `sub_6FCBD900`).
    fn may_attack(&self, attacker: UnitId, defender: UnitId) -> bool;
    /// Alignment (`0x006259B0`): 0 evil, 1 neutral, 2 good.
    fn alignment(&self, unit: UnitId) -> u8;
    /// Monster that is a hireling (R6.1 step 3).
    fn is_hireling(&self, unit: UnitId) -> bool;
    /// `justhit` (`0x005ADB00` body): a stat list with state 86 expiring
    /// at `expire` on the unit, state 86 on, and a type-12 timer at
    /// `expire` (`tick.md` §5.2).
    fn apply_justhit(&mut self, game: &mut Game, unit: UnitId, expire: i32);
}

/// Path primitives. Provider: the units session (`sim/units.md` path
/// movement).
pub trait MissilePath {
    /// The unit has a path (unit +0x2C).
    fn has_path(&self, unit: UnitId) -> bool;
    /// `0x00648690`.
    fn set_velocity(&mut self, unit: UnitId, v: i32);
    fn velocity(&self, unit: UnitId) -> i32;
    /// `0x00648B90`.
    fn set_target_unit(&mut self, unit: UnitId, target: UnitId);
    /// `0x00648AD0`.
    fn set_target_point(&mut self, unit: UnitId, x: i32, y: i32);
    /// `0x00648C30`: footprint mask.
    fn set_footprint_mask(&mut self, unit: UnitId, mask: u16);
    /// `0x00648CE0`: move-test mask.
    fn set_move_mask(&mut self, unit: UnitId, mask: u16);
    /// `0x00649970`: build the path toward the target.
    fn build(&mut self, game: &mut Game, unit: UnitId);
    /// Path acceleration (+0x88) and maximum velocity (+0x84).
    fn set_acceleration(&mut self, unit: UnitId, accel: i32, max_velocity: i32);
    /// `0x006417F0`: distance to the target point.
    fn target_distance(&self, unit: UnitId) -> i32;
    /// Unit step `0x00554CA0`: false when it returns 2 (no movement).
    fn step(&mut self, game: &mut Game, unit: UnitId) -> bool;
    /// `0x00648EB0`: collision word under the unit, all bits.
    fn collision_word(&self, game: &Game, unit: UnitId) -> u16;
    /// `0x00648F40`: subtiles crossed by the last step, in path order.
    fn crossed_subtiles(&self, unit: UnitId) -> Vec<(i32, i32)>;
}

/// Rooms and collision maps. Provider: the DRLG session.
pub trait MissileRooms {
    /// `0x00463740`: the room containing (x, y), searched from `near` and
    /// its adjacent rooms.
    fn find_room(&self, game: &Game, near: RoomId, x: i32, y: i32) -> Option<RoomId>;
    /// `0x0061AB00`: the room is in town.
    fn in_town(&self, game: &Game, room: RoomId) -> bool;
    /// `0x0064D9B0`: collision at (x, y) with `size`, masked by `mask`.
    fn collision_mask(
        &self,
        game: &Game,
        room: RoomId,
        x: i32,
        y: i32,
        size: i32,
        mask: u16,
    ) -> u16;
    /// `0x0064CB30`: collision at the subtile, masked by `mask`.
    fn collision_at(&self, game: &Game, room: RoomId, x: i32, y: i32, mask: u16) -> u16;
    /// `0x0064EBA0`: clear collision bit 0x40 under the unit with its size.
    fn clear_footprint(&mut self, game: &mut Game, unit: UnitId);
    /// The units hit by a query of size `r` (the missile's size) at
    /// subtile (x, y), in the search order of `0x00641CB0` (D2MOO
    /// `D2Common_10407`, `sim/path-placement.md` §4 rule 6); the missile
    /// code applies the collide callback to each.
    fn units_at(&self, game: &Game, room: RoomId, x: i32, y: i32, r: i32) -> Vec<UnitId>;
}

/// Damage and skill code. Provider: the combat/skills session.
pub trait MissileCombat {
    /// Damage setup `0x0059F900` (§R2.3 step 23, `missiles/damage.md`),
    /// with its draws. `skill` is the missile's stored skill (data
    /// +0x0A). Returns the missile data flag bits (+0x14) it sets
    /// (`damage.md` §2).
    fn damage_setup(
        &mut self,
        game: &mut Game,
        owner: UnitId,
        origin: Option<UnitId>,
        missile: UnitId,
        skill: i32,
        level: i32,
    ) -> u32;
    /// To-hit `0x0057D9B0(owner, defender, tohit, missile = 1)`: one draw
    /// on the owner's seed. Only called with an owner (a missing owner
    /// misses without a draw, R5 step 5).
    fn hit_test(&mut self, game: &mut Game, owner: UnitId, defender: UnitId, tohit: i32) -> bool;
    /// The rest of `0x005ADCD0` after the missile's result flags: block /
    /// dodge, hit class, hit flags, pierce percent, events and damage
    /// execution (§R6.1). `data` is the missile's data (class, data
    /// flags), passed because the store is lent out during the hit.
    fn apply_damage(
        &mut self,
        game: &mut Game,
        owner: Option<UnitId>,
        missile: UnitId,
        unit: UnitId,
        damage: &mut Damage,
        data: Option<&MissileData>,
    );
    /// Unit event 0 (hit by missile, `0x005C0C30`), also with no unit.
    fn hit_by_missile_event(&mut self, game: &mut Game, missile: UnitId, unit: Option<UnitId>);
    /// Damage-percent bonuses of the hit unit's kind (R6.2): stat 121 for
    /// demons, 122 for undead, 180 by monster type, summed.
    fn target_damage_bonus(&self, missile: UnitId, unit: UnitId) -> i32;
    /// R6.1 step 3: armor += `delta` (missile stat 120), clamped at ≥ 0.
    fn add_target_ac(&mut self, game: &mut Game, unit: UnitId, delta: i32);
}

/// Hooks of skill and monster code run by creation. Providers: the
/// combat/skills session (init callbacks), `monsters/init.md` (unique
/// mods).
pub trait MissileHooks {
    /// The parameter record's init callback (§R2.3 step 21).
    fn init_callback(&mut self, game: &mut Game, missile: UnitId, callback: u32, arg: u32);
    /// Unique-mod hook `0x005A43B0` for a monster owner (§R2.3 step 28).
    fn unique_mod_missile(&mut self, game: &mut Game, owner: UnitId, missile: UnitId);
}

/// The skills formula a server body evaluates (`skills/levels.md`
/// `eval(owner, k.<field>, k, level)`, §R9.5 / §R9.6).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SkillCalc {
    Calc1,
    Calc2,
    AuraRange,
    AuraLen,
    Calc4,
}

/// A raw skills column a body reads (`missiles/bodies.md`,
/// `bodies-2.md`; offsets by `data/fields.tsv`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SkillField {
    /// `Param1`…`Param8` (+0x148…+0x164); the index is 1…8.
    Param(u8),
    /// `aurafilter` (+0x50).
    AuraFilter,
    /// `auratargetstate` (+0x82, read as i16).
    AuraTargetState,
    /// `pettype` (+0xBE, byte).
    PetType,
}

/// What `summon_class` (`0x0056E620`, `skills/bodies.md` §6.1) answers:
/// the class (< 0: none) and the mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SummonClass {
    pub class: i32,
    pub mode: i32,
}

/// What the server-do and server-hit bodies (§R9.5, §R9.6) need beyond
/// the other seams. Providers: skills (formulas, the area scan of
/// `skills/bodies.md` §2.12), path (new-step flag, target), DRLG
/// (collision writes), units (animation frame, alive), combat (the area
/// hit). Every default is the narrowest reading: nothing happens, or the
/// value that makes the caller do nothing.
#[allow(unused_variables)]
pub trait MissileBodies {
    /// `0x0064B7C0(missile, owner, field, class, level)`: a missiles
    /// formula (`data/calc-expressions.md`).
    fn missile_calc(
        &mut self,
        game: &mut Game,
        missile: UnitId,
        owner: Option<UnitId>,
        field: u32,
        class: i32,
        level: i32,
    ) -> i32 {
        0
    }
    /// The skills record of `skill` exists.
    fn skill_exists(&self, skill: i32) -> bool {
        false
    }
    /// `eval(owner, k.<calc>, k, level)` (`skills/levels.md` §2; the
    /// owner may be none).
    fn skill_calc(
        &mut self,
        game: &mut Game,
        owner: Option<UnitId>,
        skill: i32,
        calc: SkillCalc,
        level: i32,
    ) -> i32 {
        0
    }
    /// `0x006505C0`: the path's new-step flag (path +0x34 bit 3).
    fn path_new_step(&self, unit: UnitId) -> bool {
        false
    }
    /// The unit's path target, after the refresh of `skills/bodies.md`
    /// §2.1.
    fn path_target(&mut self, game: &Game, unit: UnitId) -> Option<UnitId> {
        None
    }
    /// `0x0064CB90(room, x, y, bits)`: OR `bits` into the one collision
    /// cell at (x, y) of the room containing it, searched from `room`.
    fn or_collision(&mut self, game: &mut Game, room: RoomId, x: i32, y: i32, bits: u16) {}
    /// `0x0064EA00`: stamp `bits` with the unit's size at its position
    /// (`sim/path-placement.md` §5.1).
    fn stamp_collision(&mut self, game: &mut Game, unit: UnitId, bits: u16) {}
    /// Unit +0x44 (animation frame, 8.8).
    fn anim_frame(&self, unit: UnitId) -> i32 {
        0
    }
    fn set_anim_frame(&mut self, unit: UnitId, v: i32) {}
    /// `0x005541B0`: the unit is dead.
    fn is_dead(&self, unit: UnitId) -> bool {
        false
    }
    /// `0x0063E940` demon.
    fn is_demon(&self, unit: UnitId) -> bool {
        false
    }
    /// `0x0063E990` undead.
    fn is_undead(&self, unit: UnitId) -> bool {
        false
    }
    /// The units `scan_unit(game, owner, x, y, r, f, …, noaura 0)`
    /// (`skills/bodies.md` §2.12) hands its callback, in order.
    fn area_units(
        &mut self,
        game: &mut Game,
        owner: UnitId,
        at: (i32, i32),
        r: i32,
        f: u32,
    ) -> Vec<UnitId> {
        Vec::new()
    }
    /// `0x0056B9C0(game, owner, U, record)` on a copy of the record
    /// (§R9.6 `area_damage`).
    fn area_hit(
        &mut self,
        game: &mut Game,
        owner: UnitId,
        unit: UnitId,
        record: &crate::combat::DamageRecord,
    ) {
    }

    // ---- `missiles/bodies.md`, `missiles/bodies-2.md` --------------

    /// A raw skills column of `skill` (0 without a record).
    fn skill_field(&self, skill: i32, field: SkillField) -> i32 {
        0
    }
    /// `scan_unit(game, owner, x, y, r, f, …, noaura)` (`skills/bodies.md`
    /// §2.12) as [`Self::area_units`], with the `noaura` argument (the
    /// default has no scan for `noaura` 1).
    fn scan_units(
        &mut self,
        game: &mut Game,
        owner: UnitId,
        at: (i32, i32),
        r: i32,
        f: u32,
        noaura: bool,
    ) -> Vec<UnitId> {
        if noaura {
            Vec::new()
        } else {
            self.area_units(game, owner, at, r, f)
        }
    }
    /// The pet type count (data tables +0xBF0).
    fn pet_type_count(&self) -> i32 {
        0
    }
    /// The states count (data tables +0xC4).
    fn states_count(&self) -> i32 {
        0
    }
    /// The overlay count (data tables +0xBC0).
    fn overlay_count(&self) -> i32 {
        0
    }
    /// `phys_min(O, k, L, 1)` then `phys_max(O, k, L, 1)` (`0x00647BC0`,
    /// `0x00647D00`, `skills/levels.md` §3.3), in that order.
    fn skill_phys(
        &mut self,
        game: &mut Game,
        owner: Option<UnitId>,
        skill: i32,
        level: i32,
    ) -> (i32, i32) {
        (0, 0)
    }
    /// `elem_len(O, k, L, 1)` (`0x00644F20`, `skills/levels.md` §3.2).
    fn skill_elem_len(&mut self, game: &mut Game, owner: UnitId, skill: i32, level: i32) -> i32 {
        0
    }
    /// The param 1 of `owner`'s skill entry for `skill` (`0x006439F0`,
    /// `0x006444A0`); `None` without an entry.
    fn skill_entry_param1(&self, owner: UnitId, skill: i32) -> Option<i32> {
        None
    }
    /// Max life `0x00625D10`.
    fn max_life(&self, unit: UnitId) -> i32 {
        0
    }
    /// Max mana `0x00625D60`.
    fn max_mana(&self, unit: UnitId) -> i32 {
        0
    }
    /// `0x00621E40(unit, overlay, 0)`.
    fn overlay(&mut self, game: &mut Game, unit: UnitId, overlay: i32) {}
    /// The monster's class has the monstats2 `large` flag
    /// (`0x004638A0(class, 11)`).
    fn is_large_monster(&self, unit: UnitId) -> bool {
        false
    }
    /// The monster's class has the monstats2 `small` flag
    /// (`0x004638A0(class, 10)`; moltenboulder, §R6.3 function 14).
    fn is_small_monster(&self, unit: UnitId) -> bool {
        false
    }
    /// Pet test `0x005542C0(game, owner, unit, skill)`.
    fn is_pet(&self, game: &Game, owner: UnitId, unit: UnitId, skill: i32) -> bool {
        false
    }
    /// Ally test `0x00554D20(game, owner, unit, skill)`.
    fn is_ally(&self, game: &Game, owner: UnitId, unit: UnitId, skill: i32) -> bool {
        false
    }
    /// Ally test `0x00554DE0(game, owner, unit)` (`skills/bodies.md` §2.11
    /// flag 0x10000).
    fn ally_test(&self, game: &Game, owner: UnitId, unit: UnitId) -> bool {
        false
    }
    /// `accepts(source, unit, f)` = `0x0056B3E0` (`skills/bodies.md`
    /// §2.11).
    fn accepts(&self, game: &Game, source: UnitId, unit: UnitId, f: u32) -> bool {
        false
    }
    /// The unit's target position (`0x0056D2C0`, `skills/bodies.md` §2.4:
    /// the target unit's position, else the path target point).
    fn target_position(&mut self, game: &Game, unit: UnitId) -> Option<(i32, i32)> {
        None
    }
    /// The path target point (path +0x10, +0x12: `0x00648A00`,
    /// `0x00648A10`).
    fn path_target_point(&self, unit: UnitId) -> (i32, i32) {
        (0, 0)
    }
    /// `0x00648CF0`: path type.
    fn set_path_type(&mut self, unit: UnitId, ty: i32) {}
    /// `0x00648E70`: path distance.
    fn set_path_distance(&mut self, unit: UnitId, d: i32) {}
    /// `0x00621DC0(unit, x, y)`: the 64-step direction from the unit's
    /// position to (x, y) (`skills/bodies-3.md` §3.8, `sim/pathing.md`
    /// §8.3). Default: 0.
    fn dir64(&self, unit: UnitId, at: (i32, i32)) -> i32 {
        0
    }
    /// Path point i := (x, y) (the point array of `0x006487D0`;
    /// `skills/bodies-4.md` §2.4).
    fn set_path_point(&mut self, unit: UnitId, i: i32, at: (u16, u16)) {}
    /// The path's point count := n (`0x00648790`).
    fn set_path_point_count(&mut self, unit: UnitId, n: i32) {}
    /// `0x00650BE0(path, unit, room, x, y)` (`sim/path-placement.md` §6).
    fn path_teleport(
        &mut self,
        game: &mut Game,
        unit: UnitId,
        room: Option<RoomId>,
        x: i32,
        y: i32,
    ) {
    }
    /// `0x0061AED0(room, 1)`: refresh the room.
    fn refresh_room(&mut self, game: &mut Game, room: RoomId) {}
    /// `0x0064E260(room, from, to, mask)`: the line test reports a hit.
    fn line_hits(
        &self,
        game: &Game,
        room: RoomId,
        from: (i32, i32),
        to: (i32, i32),
        mask: u16,
    ) -> bool {
        false
    }
    /// The room seed (room +0x6C); `None` without one.
    fn room_seed(&mut self, game: &mut Game, room: RoomId) -> Option<&mut Seed> {
        None
    }
    /// Skill server-do `index` with `caster` as the caster
    /// (`0x0056D810`, `skills/use.md`: index ≤ 190 unsigned and a non-null
    /// entry, else nothing); its result is ignored.
    fn skill_srv_do(
        &mut self,
        game: &mut Game,
        caster: UnitId,
        index: i32,
        skill: i32,
        level: i32,
    ) {
    }
    /// `shout_state(game, unit, owner, skill, level)` (`0x005D8290`,
    /// `skills/bodies.md` §6.8).
    fn shout_state(
        &mut self,
        game: &mut Game,
        unit: UnitId,
        owner: UnitId,
        skill: i32,
        level: i32,
    ) {
    }
    /// The unit's stat list of state `s` (`0x006256B0`); its expiry frame
    /// (list +0x18, `0x00626090`) when there is one.
    fn state_list_expiry(&self, unit: UnitId, s: i32) -> Option<i32> {
        None
    }
    /// §12 step 6: a fresh list (game pool, flags 2, expiry `expire`,
    /// `owner`'s type and GUID), state id `s`, remove callback
    /// `0x0056E900`, attached to the unit, state `s` on. False when the
    /// allocation fails.
    fn new_state_list(
        &mut self,
        game: &mut Game,
        unit: UnitId,
        s: i32,
        expire: i32,
        owner: UnitId,
    ) -> bool {
        false
    }
    /// `aura_fill(unit, list of s, k record, k, L)` (`0x005C6CC0`,
    /// `skills/bodies.md` §2.6) on the unit's existing list of `s`.
    fn aura_fill(&mut self, game: &mut Game, unit: UnitId, s: i32, skill: i32, level: i32) {}
    /// `0x00639E30(unit, s, 1)`: mark state `s` changed.
    fn mark_state_changed(&mut self, unit: UnitId, s: i32) {}
    /// `0x006260B0`: the expiry of the unit's list of `s` (nothing
    /// without one).
    fn set_state_list_expiry(&mut self, unit: UnitId, s: i32, expire: i32) {}
    /// `apply_state` (`0x0056E970`, `skills/bodies.md` §2.7) with source
    /// `owner`, target `unit`, skill, level, `duration`, stat field 0
    /// value 0, state `s`, callback 0. True when it made a list.
    #[allow(clippy::too_many_arguments)]
    fn apply_state(
        &mut self,
        game: &mut Game,
        owner: UnitId,
        unit: UnitId,
        skill: i32,
        level: i32,
        duration: i32,
        s: i32,
    ) -> bool {
        false
    }
    /// Terror install `0x005DDD00(game, source, unit, skill, a, b)`
    /// (`monsters/ai.md`).
    #[allow(clippy::too_many_arguments)]
    fn terror(
        &mut self,
        game: &mut Game,
        source: UnitId,
        unit: UnitId,
        skill: i32,
        a: i32,
        b: i32,
    ) {
    }
    /// `summon_class(O, k, L, &mode)` (`0x0056E620`, `skills/bodies.md`
    /// §6.1).
    fn summon_class(
        &mut self,
        game: &mut Game,
        owner: UnitId,
        skill: i32,
        level: i32,
    ) -> SummonClass {
        SummonClass { class: -1, mode: 0 }
    }
    /// `summon_spawn` (`0x0056D940`, `skills/bodies.md` §6.2) with flags
    /// 0xD, AI special state 0, pet max 0.
    #[allow(clippy::too_many_arguments)]
    fn summon_spawn(
        &mut self,
        game: &mut Game,
        owner: UnitId,
        class: i32,
        mode: i32,
        at: (i32, i32),
        pet_type: i32,
    ) -> Option<UnitId> {
        None
    }
    /// `missiles/bodies-2.md` §33 step 8 on piece `piece` of anchor
    /// `anchor`, in order: owner data (anchor GUID, 1, 0, 0)
    /// `0x0058F030`; minion list `0x0058F100`; umod 15 `0x005A4850`;
    /// `skill_stats(game, owner, piece, k, L, 0)` `0x005C4470`; stored
    /// owner := `owner` `0x00621CE0`; `0x005B1990(game, piece, 0, 9)`.
    fn bind_bone_wall_piece(
        &mut self,
        game: &mut Game,
        owner: UnitId,
        anchor: UnitId,
        piece: UnitId,
        skill: i32,
        level: i32,
    ) {
    }
    /// The portal object `0x0056D130(game, owner, room, x, y, level, 0,
    /// class, 1)` (`world/quests.md`).
    #[allow(clippy::too_many_arguments)]
    fn create_portal(
        &mut self,
        game: &mut Game,
        owner: Option<UnitId>,
        room: Option<RoomId>,
        at: (i32, i32),
        level: i32,
        class: i32,
    ) {
    }
    /// Sound event `0x00553380(unit, id, 0)`.
    fn unit_sound(&mut self, game: &mut Game, unit: UnitId, id: i32) {}
    /// The chest drop `0x00585E00` with operate context {game, chest,
    /// operator none, chest seed, chest class} (`items/treasure.md` §4,
    /// Q = 4).
    fn chest_drop(&mut self, game: &mut Game, chest: UnitId) {}
    /// Floor drop spot `0x00555DA0(room, (x, y), &out, size 1, fallback
    /// 1)` (`items/treasure.md` §7 step 2).
    fn floor_drop_spot(
        &mut self,
        game: &Game,
        room: RoomId,
        at: (i32, i32),
    ) -> Option<(RoomId, i32, i32)> {
        None
    }
    /// The gold item request of `missiles/bodies-2.md` §43 step 4.3
    /// (`0x00558D90`, `items/generation.md` §3) for `missile` at the spot.
    fn create_gold(&mut self, game: &mut Game, missile: UnitId, room: RoomId, at: (i32, i32)) {}
    /// Unit mode (+0x10) of a non-missile unit.
    fn unit_mode(&self, unit: UnitId) -> i32 {
        0
    }
    /// `0x00624690(unit, mode)`.
    fn set_unit_mode(&mut self, unit: UnitId, mode: i32) {}
    /// The objects record's `FrameCnt1` (+0xDC) of the object's class
    /// (`0x00640E90`); `None` without a record.
    fn object_frame_cnt1(&self, unit: UnitId) -> Option<i32> {
        None
    }
    /// The active room's sub-tile rectangle (x, y, w, h; active room
    /// +0x4C, `drlg/rooms.md` §1) read by the unit find (`bodies-2.md`
    /// §44, `0x0065A6B0`); `None` without one.
    fn room_subtiles(&self, game: &Game, room: RoomId) -> Option<(i32, i32, i32, i32)> {
        None
    }
    /// `0x005D0C40(game, missile, unit, skill, level, last)` (Redemption's
    /// per-corpse effect; `missiles/bodies-2.md` Open question 4).
    #[allow(clippy::too_many_arguments)]
    fn redemption_effect(
        &mut self,
        game: &mut Game,
        missile: UnitId,
        unit: UnitId,
        skill: i32,
        level: i32,
        last: bool,
    ) {
    }
    /// The `mon` list of the levels record of the room's level
    /// (`0x0061DB70(0x0061A1B0(room))`, +0x33 count, +0x36 entries).
    fn level_mon_list(&self, game: &Game, room: RoomId) -> Vec<i32> {
        Vec::new()
    }
    /// The monstats `isSpawn` flag of `class`; `None` for an invalid class
    /// (the original reads through a null record).
    fn monster_is_spawn(&self, class: i32) -> Option<bool> {
        None
    }
    /// The monster create request (`monsters/init.md` §2) {room, class,
    /// mode 1, GUID 0, x, y, spread 5, flags 0} → `0x005B2A00`; true when
    /// created.
    fn create_monster(
        &mut self,
        game: &mut Game,
        room: RoomId,
        class: i32,
        at: (i32, i32),
    ) -> bool {
        false
    }
    /// The monstats count.
    fn monstats_count(&self) -> i32 {
        0
    }
    /// `0x0054E600(game, room, class, x, y, mode)`
    /// (`monsters/population.md` §11.2).
    fn spawn_monster(
        &mut self,
        game: &mut Game,
        room: Option<RoomId>,
        class: i32,
        at: (i32, i32),
        mode: i32,
    ) {
    }
    /// The rabies poison `0x005C7DB0(game, owner, unit, t, skill, level)`
    /// (`missiles/bodies-2.md` Open question 5).
    #[allow(clippy::too_many_arguments)]
    fn rabies_poison(
        &mut self,
        game: &mut Game,
        owner: UnitId,
        unit: UnitId,
        t: i32,
        skill: i32,
        level: i32,
    ) {
    }
    /// The quest test `0x005444B0(game, id)` (no entry with `id` → true;
    /// else its byte +9 = 1).
    fn quest_test(&self, game: &Game, id: i32) -> bool {
        false
    }
    /// `0x0058E920(game, room, missile)` (Tyrael's spawn; `missiles/
    /// bodies-2.md` Open question 7).
    fn spawn_tyrael(&mut self, game: &mut Game, room: Option<RoomId>, missile: UnitId) {}
}

/// Everything missile code needs.
pub trait MissileWorld:
    MissileUnits + MissilePath + MissileRooms + MissileCombat + MissileHooks + MissileBodies
{
}

impl<
        T: MissileUnits
            + MissilePath
            + MissileRooms
            + MissileCombat
            + MissileHooks
            + MissileBodies
            + ?Sized,
    > MissileWorld for T
{
}
