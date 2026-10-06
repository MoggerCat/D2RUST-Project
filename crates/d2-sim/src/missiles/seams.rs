// Spec: specs/missiles/missiles.md (Related specs; the seams to other systems)
//! What missile code needs from systems owned by other specs. Each trait
//! names its expected provider; tests use small fakes. Nothing here
//! decides missile behaviour: the rules stay in the missile modules.

use crate::game::Game;
use crate::rng::Seed;
use crate::units::{RoomId, UnitId};

use super::hit::Damage;

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
    /// The units on subtile (x, y) in the search order of `0x00641CB0`
    /// (D2MOO `D2Common_10407`); the missile code applies the collide
    /// callback to each.
    fn units_at(&self, game: &Game, room: RoomId, x: i32, y: i32) -> Vec<UnitId>;
}

/// Damage and skill code. Provider: the combat/skills session.
pub trait MissileCombat {
    /// Damage setup `0x0059F900` (§R2.3 step 23), with its draws.
    fn damage_setup(
        &mut self,
        game: &mut Game,
        owner: UnitId,
        origin: Option<UnitId>,
        missile: UnitId,
        level: i32,
    );
    /// To-hit `0x0057D9B0(owner, defender, tohit, missile = 1)`: one draw
    /// on the owner's seed. Only called with an owner (a missing owner
    /// misses without a draw, R5 step 5).
    fn hit_test(&mut self, game: &mut Game, owner: UnitId, defender: UnitId, tohit: i32) -> bool;
    /// Server-damage function `index` (1…14; `0x0073C960`).
    fn srv_dmg(
        &mut self,
        game: &mut Game,
        index: i16,
        missile: UnitId,
        unit: UnitId,
        damage: &mut Damage,
    );
    /// The rest of `0x005ADCD0` after the missile's result flags: block /
    /// dodge, hit class, hit flags, pierce percent, events and damage
    /// execution (§R6.1).
    fn apply_damage(
        &mut self,
        game: &mut Game,
        owner: Option<UnitId>,
        missile: UnitId,
        unit: UnitId,
        damage: &mut Damage,
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
        game: &Game,
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
