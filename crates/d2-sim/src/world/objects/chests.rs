// Spec: specs/world/objects.md §8
//! Chests and breakables (§8.1, §8.2) and traps (§8.3).
//!
//! Every roll here is on the object-control seed (`ctl.seed`, spec
//! Randomness items 4, 5, 7); the item drops draw on the dropping unit's
//! seed inside the [`ChestWorld::chest_drop`] provider
//! (`items/treasure.md` §4) and the mode changes draw on the object's unit
//! seed through [`set_mode`] (§4).

use crate::units::{RoomId, UnitId};

use super::{
    clear_selectable, oevent, schedule_endanim, set_mode, sound, ObjectControl, ObjectError,
    ObjectTables, ObjectWorld, Operate, Operator, ASSASSIN,
};

#[cfg(test)]
mod tests;

/// Unit types (`sim/units.md` §1) the chest code tests.
pub mod unit_type {
    pub const PLAYER: u8 = 0;
    pub const MONSTER: u8 = 1;
    pub const OBJECT: u8 = 2;
    pub const ITEM: u8 = 4;
    pub const TILE: u8 = 5;
}

/// The special chest class (§8.1 rule 4).
pub const SPECIAL_CHEST: u16 = 397;
/// The exploding barrel class chained by the room walk (§8.2, fn 7).
pub const EXPLODING_BARREL_CLASS: u16 = 11;
/// Trap monster that never arms in act I (§8.3).
pub const ACT1_SKIPPED_TRAP_MONSTER: u32 = 234;
/// Trap arm delay in frames (§8.3, `0x00582510`).
pub const TRAP_DELAY: i32 = 35;
/// Trap handler bound (`0x00732D14`, §8.3).
pub const TRAP_BOUND: u8 = 10;
/// Trap monster spawn argument of `0x00582280` (§8.2, §8.3).
pub const TRAP_SPAWN_ARG: u32 = 8;
/// Player mode that the exploding barrel skips (§8.2 fn 7).
pub const PLAYER_DEATH_MODE: u8 = 17;
/// Monster mode that the exploding barrel skips (§8.2 fn 7).
pub const MONSTER_DEATH_MODE: u8 = 12;
/// Flag of `0x005B3090` at the object's spot (§8.3).
pub const TRAP_SPOT_FLAG: u32 = 0x88;
/// Collision mask of the free-spot search of `0x00582420` (§8.3).
pub const TRAP_FREE_SPOT_MASK: u32 = 0x3F11;
/// Trap objects of handlers 5 and 7 (§8.3).
pub const TRAP_FIRE_OBJECT: u16 = 162;
pub const TRAP_FIRE_OBJECT_2: u16 = 160;
/// Spark byte of the trap fire objects (§1, §8.3).
pub const SPARK_TRAP_FIRE: u8 = 2;

/// A 4-character item code as the original stores it (little-endian
/// dword of the four bytes).
pub const fn item_code(c: &[u8; 4]) -> u32 {
    u32::from_le_bytes(*c)
}
pub const GOLD: u32 = item_code(b"gld ");
pub const HP3: u32 = item_code(b"hp3 ");
pub const MP3: u32 = item_code(b"mp3 ");

/// The chest seams beyond [`ObjectWorld`]. Every default is the narrowest
/// reading: nothing happens, or none.
#[allow(unused_variables)]
pub trait ChestWorld: ObjectWorld {
    /// `0x00585B90` (`items/treasure.md` §4): the chest drop `D(Q)` with
    /// the operate record; returns the first item dropped, or none.
    fn chest_drop(&mut self, op: &Operate, q: u8) -> Option<UnitId> {
        None
    }
    /// Unit type (`sim/units.md` §1): used by the magic test `0x0062A0F0`
    /// (type 4 = item) and the exploding barrel's room walk (§8.2).
    fn unit_type(&self, unit: UnitId) -> Option<u8> {
        None
    }
    /// Item quality (item data, `items/` specs): the magic test
    /// `0x0062A0F0` reads 4…9 as magic.
    fn item_quality(&self, item: UnitId) -> Option<u8> {
        None
    }
    /// `0x00585970(game, object, code, 0)` (items spec): the code drop
    /// `C(code)`, one item of that 4-character code at the object.
    fn code_drop(&mut self, object: UnitId, code: u32) -> Option<UnitId> {
        None
    }
    /// `0x00559A30` (items spec): drop the object's drop item code (unit
    /// +0xB8) at the object (§8.1 rule 7).
    fn drop_item_code(&mut self, object: UnitId, code: u32) {}
    /// `0x005474C0`: the trap monster id for the object (per-level cache,
    /// D2MOO `OBJRGN_GetTrapMonsterId`; open question 3 of
    /// `world/objects.md`, owner monsters spec). Draws, if any, are the
    /// provider's.
    fn trap_monster_id(&mut self, object: UnitId) -> Option<u32> {
        None
    }
    /// `0x00582280(…, monster, arg)`: spawn one trap monster for the
    /// object (§8.2 casket and barrel, §8.3 handlers 8 and 9 with arg 8;
    /// monsters spec).
    fn spawn_trap_monster(&mut self, object: UnitId, monster: u32, arg: u32) {}
    /// `0x005B3090` with flag 0x88 (monsters spec): spawn `monster` at the
    /// object's own spot (x, y); none when the spot is refused (§8.3
    /// `0x00582420`).
    fn spawn_monster_at_spot(
        &mut self,
        room: RoomId,
        monster: u32,
        x: i32,
        y: i32,
        flag: u32,
    ) -> Option<UnitId> {
        None
    }
    /// The free-spot search of `0x00582420` with collision mask 0x3F11
    /// (path placement): a free spot near (x, y), or none (§8.3).
    fn free_spot(&self, room: RoomId, x: i32, y: i32, mask: u32) -> Option<(RoomId, i32, i32)> {
        None
    }
    /// `0x005B2F20` (monsters spec): spawn `monster` at (x, y) in `room`
    /// (§8.3 `0x00582420`, after the free-spot search).
    fn spawn_monster(&mut self, room: RoomId, monster: u32, x: i32, y: i32) -> Option<UnitId> {
        None
    }
    /// `0x00580A70` (skills spec): start the player's skill 1
    /// (`0x006439B0`) on the barrel (§8.2 fn 5; D2MOO: the player's
    /// attack mode toward the object).
    fn start_player_skill_on(&mut self, player: UnitId, object: UnitId) {}
    /// The units of `room` in the room's list order (`0x00584240` walk,
    /// §8.2 fn 7; DRLG / unit lists).
    fn room_units(&self, room: RoomId) -> Vec<UnitId> {
        Vec::new()
    }
    /// The range test of `0x00584240` (§8.2 fn 7): `unit` is within
    /// `range` of `object` (range 3 for damage, distance 2 for chained
    /// barrels). Metric and bound inclusiveness are the provider's (not
    /// stated in the spec).
    fn within(&self, object: UnitId, unit: UnitId, range: i32) -> bool {
        false
    }
    /// `0x005DFA00` (combat spec): trap damage from `object` to `target`
    /// (§8.2 fn 7).
    fn trap_damage(&mut self, object: UnitId, target: UnitId) {}
    /// `0x005A43E0` (monsters spec): spawn the first monster of the
    /// level's region list that can walk, at the object (§8.2 fn 68).
    fn spawn_region_monster(&mut self, object: UnitId) {}
    /// The "inside the room" test of `0x00582380` (§8.3 handlers 5, 7):
    /// (x, y) lies in `room` (DRLG).
    fn in_room(&self, room: RoomId, x: i32, y: i32) -> bool {
        false
    }
}

/// The magic test `0x0062A0F0` (§8 common pieces): the item exists, is
/// type 4 and its quality is 4…9.
pub fn is_magic<W: ChestWorld>(w: &W, item: Option<UnitId>) -> bool {
    item.is_some_and(|i| {
        w.unit_type(i) == Some(unit_type::ITEM)
            && w.item_quality(i).is_some_and(|q| (4..=9).contains(&q))
    })
}

/// Operate 1, 3, 4, 5, 7, 14, 68 (§8.1, §8.2).
pub fn operate<W: ChestWorld>(
    ctl: &mut ObjectControl,
    t: &ObjectTables,
    w: &mut W,
    op: &Operate,
) -> Result<i32, ObjectError> {
    match op.operate_fn {
        4 => chest(ctl, t, w, op),
        1 => casket(ctl, t, w, op),
        3 => urn(ctl, t, w, op),
        5 => barrel(ctl, t, w, op),
        7 => {
            if w.mode(op.object) == 0 {
                exploding_barrel(ctl, t, w, op.object)?;
            }
            Ok(1)
        }
        14 => corpse(ctl, t, w, op),
        68 => evil_urn(ctl, t, w, op),
        _ => Ok(0),
    }
}

// ------------------------------------------------------------------ §8.1

/// Operate 4 `0x00585F60` (§8.1).
fn chest<W: ChestWorld>(
    ctl: &mut ObjectControl,
    t: &ObjectTables,
    w: &mut W,
    op: &Operate,
) -> Result<i32, ObjectError> {
    let obj = op.object;
    // Rule 1.
    if w.mode(obj) != 0 {
        return Ok(1);
    }
    let d = *ctl.get(obj)?;
    // Rule 2.
    let locked = d.interact & 0x80 != 0;
    let picks = if locked {
        // TODO(objects.md §8.1 r2): a locked chest operated with no
        // operator is not described; read as "no key" without a sound.
        let Some(p) = op.operator else {
            return Ok(1);
        };
        let pass = w.operator(p) == Operator::Player(ASSASSIN) || w.key_test(p);
        if !pass {
            w.sound(p, sound::LOCKED, None, false);
            return Ok(1);
        }
        w.sound(p, sound::UNLOCK, None, true);
        2
    } else {
        1
    };
    // Rule 3.
    let sparkling = d.spark & 1 != 0;
    let q = if sparkling {
        if ctl.seed.roll(100) < 5 {
            6
        } else {
            4
        }
    } else {
        0
    };
    if op.class == SPECIAL_CHEST {
        // Rule 4 (edge case 7: picks, sparkle and Q ignored).
        special_chest(ctl, w, op);
    } else {
        // Rule 5.
        let r = ctl.seed.roll(100);
        if !(r < 25 && !sparkling && !locked) {
            let mut magic = false;
            for _ in 0..picks {
                let i = w.chest_drop(op, q);
                magic |= is_magic(w, i);
            }
            if sparkling && !magic {
                for _ in 0..10 {
                    let i = w.chest_drop(op, q);
                    if is_magic(w, i) {
                        break;
                    }
                }
            }
        }
    }
    // Rules 6, 7.
    open(ctl, t, w, obj)
}

/// §8.1 rule 4: the class-397 drops (everything up to "open").
fn special_chest<W: ChestWorld>(ctl: &mut ObjectControl, w: &mut W, op: &Operate) {
    let r = ctl.seed.roll(10000);
    let opened = if r < 1200 {
        let q = if r < 200 {
            7
        } else if r < 600 {
            5
        } else {
            6
        };
        let mut opened = false;
        for _ in 0..2 {
            let i = w.chest_drop(op, q);
            if i.is_none() {
                break;
            }
            if is_magic(w, i) {
                opened = true;
                break;
            }
        }
        opened
    } else if r < 3200 {
        let (mut items, mut magic) = (0, 0);
        for _ in 0..10 {
            let i = w.chest_drop(op, 4);
            if i.is_some() {
                items += 1;
            }
            if is_magic(w, i) {
                magic += 1;
                if magic == 3 {
                    break;
                }
            }
        }
        items > 0
    } else if r < 6200 {
        let (mut m, mut n) = (0, 0);
        for _ in 0..10 {
            let i = w.chest_drop(op, 4);
            if is_magic(w, i) {
                m += 1;
                if m == 2 {
                    break;
                }
            } else if i.is_some() {
                n += 1;
            }
        }
        if n == 0 && w.chest_drop(op, 0).is_some() {
            n = 1;
        }
        if n < 7 {
            for _ in 0..7 - n {
                w.code_drop(op.object, GOLD);
            }
        }
        true
    } else {
        false
    };
    if opened {
        return;
    }
    // Tail.
    let mut k = 0;
    for _ in 0..10 {
        let i = w.chest_drop(op, 4);
        if is_magic(w, i) {
            break;
        }
        // TODO(objects.md §8.1 r4 tail): a null drop is read as neither
        // "magic" nor "non-magic": the loop goes on without counting.
        if i.is_some() {
            k += 1;
        }
    }
    if k < 4 {
        for _ in 0..4 - k {
            w.chest_drop(op, 0);
        }
    }
    for (code, n) in [(GOLD, 5), (HP3, 2), (MP3, 2)] {
        for _ in 0..n {
            w.code_drop(op.object, code);
        }
    }
}

/// "Open" (§8.1 rule 7): the end of the chest operate. Returns 1.
fn open<W: ChestWorld>(
    ctl: &mut ObjectControl,
    t: &ObjectTables,
    w: &mut W,
    obj: UnitId,
) -> Result<i32, ObjectError> {
    let d = *ctl.get(obj)?;
    let o = t.object(d.class)?;
    if o.mode1 != 0 {
        set_mode(t, w, obj, d.class, 1, true)?;
        schedule_endanim(w, o, obj);
    } else {
        set_mode(t, w, obj, d.class, 2, true)?;
    }
    clear_selectable(w, obj);
    if d.drop_code != 0 {
        w.drop_item_code(obj, d.drop_code);
    }
    trap_arm(ctl, t, w, obj)?;
    Ok(1)
}

// ------------------------------------------------------------------ §8.2

/// Mode 1, clear 0x2, ENDANIM: the common opening of the breakables.
fn break_open<W: ChestWorld>(
    t: &ObjectTables,
    w: &mut W,
    obj: UnitId,
    class: u16,
) -> Result<(), ObjectError> {
    set_mode(t, w, obj, class, 1, true)?;
    clear_selectable(w, obj);
    schedule_endanim(w, t.object(class)?, obj);
    Ok(())
}

/// "Footprint as casket": `HasCollision1` = 0 → free the footprint.
fn casket_footprint<W: ChestWorld>(
    t: &ObjectTables,
    w: &mut W,
    obj: UnitId,
    class: u16,
) -> Result<(), ObjectError> {
    if t.object(class)?.hascollision1 == 0 {
        w.free_footprint(obj);
    }
    Ok(())
}

/// `roll(10000)` ≥ 8192 (`& 0xFFFFE000` ≠ 0) → one trap monster
/// (`0x005474C0`, spawn `0x00582280` arg 8).
fn maybe_trap_monster<W: ChestWorld>(ctl: &mut ObjectControl, w: &mut W, obj: UnitId) {
    if ctl.seed.roll(10000) & 0xFFFF_E000 != 0 {
        if let Some(m) = w.trap_monster_id(obj) {
            w.spawn_trap_monster(obj, m, TRAP_SPAWN_ARG);
        }
    }
}

// TODO(objects.md §8.2): the breakables' return value on their main path
// is not stated (only "else return 1"); 1 is returned on every path.

/// Operate 1 `0x00586410`: casket.
fn casket<W: ChestWorld>(
    ctl: &mut ObjectControl,
    t: &ObjectTables,
    w: &mut W,
    op: &Operate,
) -> Result<i32, ObjectError> {
    let obj = op.object;
    if w.mode(obj) != 0 {
        return Ok(1);
    }
    // Edge case 3: no item → nothing changes.
    if w.chest_drop(op, 0).is_none() {
        return Ok(1);
    }
    break_open(t, w, obj, op.class)?;
    maybe_trap_monster(ctl, w, obj);
    casket_footprint(t, w, obj, op.class)?;
    trap_arm(ctl, t, w, obj)?;
    Ok(1)
}

/// Operate 3 `0x005866C0`: urn.
fn urn<W: ChestWorld>(
    ctl: &mut ObjectControl,
    t: &ObjectTables,
    w: &mut W,
    op: &Operate,
) -> Result<i32, ObjectError> {
    let obj = op.object;
    if w.mode(obj) != 0 {
        return Ok(1);
    }
    break_open(t, w, obj, op.class)?;
    if ctl.seed.roll(100) <= 20 {
        w.chest_drop(op, 0);
    }
    casket_footprint(t, w, obj, op.class)?;
    trap_arm(ctl, t, w, obj)?;
    Ok(1)
}

/// Operate 5 `0x005868A0`: barrel (no trap arm).
fn barrel<W: ChestWorld>(
    ctl: &mut ObjectControl,
    t: &ObjectTables,
    w: &mut W,
    op: &Operate,
) -> Result<i32, ObjectError> {
    let obj = op.object;
    if w.mode(obj) != 0 {
        return Ok(1);
    }
    if let Some(p) = op.operator {
        if let Operator::Player(_) = w.operator(p) {
            w.start_player_skill_on(p, obj);
        }
    }
    // TODO(objects.md §8.2 fn 5): "clear 0x2 (when the object exists)":
    // the object is read as always existing here.
    set_mode(t, w, obj, op.class, 1, true)?;
    clear_selectable(w, obj);
    w.free_footprint(obj);
    maybe_trap_monster(ctl, w, obj);
    if ctl.seed.roll(100) <= 20 {
        w.chest_drop(op, 0);
    }
    schedule_endanim(w, t.object(op.class)?, obj);
    Ok(1)
}

/// Operate 7 `0x00584330` (mode 0 checked by the caller): exploding
/// barrel, with the room walk `0x00584240`. Chained barrels explode inside
/// the walk (edge case 11).
fn exploding_barrel<W: ChestWorld>(
    ctl: &mut ObjectControl,
    t: &ObjectTables,
    w: &mut W,
    obj: UnitId,
) -> Result<(), ObjectError> {
    let class = ctl.get(obj)?.class;
    set_mode(t, w, obj, class, 1, true)?;
    clear_selectable(w, obj);
    if let Some(room) = w.room(obj) {
        for u in w.room_units(room) {
            if u == obj {
                continue;
            }
            match w.unit_type(u) {
                Some(unit_type::PLAYER) => {
                    if w.mode(u) != PLAYER_DEATH_MODE && w.within(obj, u, 3) {
                        w.trap_damage(obj, u);
                    }
                }
                Some(unit_type::MONSTER) => {
                    if w.mode(u) != MONSTER_DEATH_MODE && w.within(obj, u, 3) {
                        w.trap_damage(obj, u);
                    }
                }
                Some(unit_type::OBJECT) => {
                    let chained = ctl
                        .data
                        .get(&u)
                        .is_some_and(|d| d.class == EXPLODING_BARREL_CLASS);
                    if chained && w.mode(u) == 0 && w.within(obj, u, 2) {
                        exploding_barrel(ctl, t, w, u)?;
                    }
                }
                _ => {}
            }
        }
    }
    w.free_footprint(obj);
    schedule_endanim(w, t.object(class)?, obj);
    Ok(())
}

/// Operate 14 `0x005867A0`: corpse / crate.
fn corpse<W: ChestWorld>(
    ctl: &mut ObjectControl,
    t: &ObjectTables,
    w: &mut W,
    op: &Operate,
) -> Result<i32, ObjectError> {
    let obj = op.object;
    if w.mode(obj) != 0 {
        return Ok(1);
    }
    w.chest_drop(op, 0);
    set_mode(t, w, obj, op.class, 1, true)?;
    schedule_endanim(w, t.object(op.class)?, obj);
    clear_selectable(w, obj);
    casket_footprint(t, w, obj, op.class)?;
    trap_arm(ctl, t, w, obj)?;
    Ok(1)
}

/// Operate 68 `0x00586520`: evil urn.
fn evil_urn<W: ChestWorld>(
    ctl: &mut ObjectControl,
    t: &ObjectTables,
    w: &mut W,
    op: &Operate,
) -> Result<i32, ObjectError> {
    let obj = op.object;
    if w.mode(obj) != 0 {
        return Ok(1);
    }
    // Edge case 3: no item → nothing changes.
    if w.chest_drop(op, 0).is_none() {
        return Ok(1);
    }
    break_open(t, w, obj, op.class)?;
    if ctl.seed.roll(255) <= t.object(op.class)?.parm7 {
        w.spawn_region_monster(obj);
    }
    casket_footprint(t, w, obj, op.class)?;
    trap_arm(ctl, t, w, obj)?;
    Ok(1)
}

// ------------------------------------------------------------------ §8.3

/// The level's act is act I (`levels.Act` = 0).
fn in_act1(t: &ObjectTables, level: u32) -> bool {
    t.level(level).is_some_and(|l| l.act == 0)
}

/// Trap arm `0x00582510` with t = `InteractType` & 0x7F (§8.3).
pub fn trap_arm<W: ChestWorld>(
    ctl: &mut ObjectControl,
    t: &ObjectTables,
    w: &mut W,
    obj: UnitId,
) -> Result<(), ObjectError> {
    let ty = ctl.get(obj)?.interact & 0x7F;
    if ty >= TRAP_BOUND || ty == 0 {
        return Ok(());
    }
    if ty == 8 || ty == 9 {
        let m = w.trap_monster_id(obj);
        let level = w.level(obj).unwrap_or(0);
        if m == Some(ACT1_SKIPPED_TRAP_MONSTER) && in_act1(t, level) {
            return Ok(());
        }
    }
    let at = w.frame() + TRAP_DELAY;
    w.schedule(obj, oevent::TRAP, at);
    w.sound(obj, sound::TRAP_ARMED, None, false);
    Ok(())
}

/// The handler number event 4 runs for trap `ty` at level `level`, after
/// the substitutions (§8.3); `None`: nothing runs.
pub fn trap_handler(ty: u8, level: u32) -> Option<u8> {
    if ty >= TRAP_BOUND || ty == 0 {
        return None;
    }
    let sub = (ty == 8 && level >= 75)
        || (ty == 3 && level < 40 && level != 25)
        || ((ty == 1 || ty == 4) && level < 40);
    Some(if sub { 2 } else { ty })
}

/// Event 4 `0x005817A0` (§8.3).
pub fn trap_event<W: ChestWorld>(
    ctl: &mut ObjectControl,
    _t: &ObjectTables,
    w: &mut W,
    obj: UnitId,
) -> Result<(), ObjectError> {
    let ty = ctl.get(obj)?.interact & 0x7F;
    let level = w.level(obj).unwrap_or(0);
    let Some(h) = trap_handler(ty, level) else {
        return Ok(());
    };
    match h {
        1 => trap_monster_at(w, obj, 330),
        2 | 6 => trap_monster_at(w, obj, 326),
        3 => trap_monster_at(w, obj, 329),
        4 => trap_monster_at(w, obj, 369),
        5 | 7 => trap_fire(ctl, w, obj),
        8 | 9 => {
            // TODO(objects.md §8.3, open question 3): whether the trap
            // monster id is read before or after the control-seed step,
            // and whether it draws, is not stated; read here after it.
            let n = 1 + (ctl.seed.step() & 1);
            if let Some(m) = w.trap_monster_id(obj) {
                for _ in 0..n {
                    w.spawn_trap_monster(obj, m, TRAP_SPAWN_ARG);
                }
            }
        }
        _ => {}
    }
    Ok(())
}

/// `0x00582420`: a trap monster at the object's spot, else at a free spot.
fn trap_monster_at<W: ChestWorld>(w: &mut W, obj: UnitId, monster: u32) {
    let Some(room) = w.room(obj) else {
        return;
    };
    let (x, y) = w.position(obj);
    if w.spawn_monster_at_spot(room, monster, x, y, TRAP_SPOT_FLAG)
        .is_some()
    {
        return;
    }
    if let Some((r, fx, fy)) = w.free_spot(room, x, y, TRAP_FREE_SPOT_MASK) {
        w.spawn_monster(r, monster, fx, fy);
    }
}

/// `0x00582380`: trap handlers 5 and 7 (fire objects).
fn trap_fire<W: ChestWorld>(ctl: &mut ObjectControl, w: &mut W, obj: UnitId) {
    let Some(room) = w.room(obj) else {
        return;
    };
    let (x, y) = w.position(obj);
    // TODO(objects.md §8.3 handlers 5, 7): the allocation mode of objects
    // 162 and 160 is not stated; mode 0 is used.
    if let Some(o) = w.allocate_object(room, TRAP_FIRE_OBJECT, x, y, 0) {
        if let Some(d) = ctl.data.get_mut(&o) {
            d.spark = SPARK_TRAP_FIRE;
        }
    }
    if w.in_room(room, x + 1, y) {
        if let Some(o) = w.allocate_object(room, TRAP_FIRE_OBJECT_2, x + 1, y, 0) {
            if let Some(d) = ctl.data.get_mut(&o) {
                d.spark = SPARK_TRAP_FIRE;
            }
        }
    }
}
