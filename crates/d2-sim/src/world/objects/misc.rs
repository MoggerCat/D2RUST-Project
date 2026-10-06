// Spec: specs/world/objects.md §10–§13, §14 rule 2
//! Doors (§10), wells (§11, event 2), portals (§12) and torches (§13).

use crate::units::UnitId;

use super::{
    clear_selectable, oflags, set_mode, set_object_mode, sound, ObjectControl, ObjectError,
    ObjectHost, ObjectTables, ObjectWorld, Operate, Operator,
};

#[cfg(test)]
mod tests;

/// Door debounce in host ticks (§10 rule 1, `0x00581D9F`).
pub const DOOR_DEBOUNCE: u32 = 500;
/// Door re-close mask: player 0x80, monster 0x100, 0x8000 (§10,
/// `0x00581DF8`).
pub const DOOR_CLEAR_MASK: u16 = 0x8180;
/// Door corpse mask (§10, `0x00581E2E`).
pub const DOOR_CORPSE_MASK: u16 = 0x8000;
/// Portal hostile delay in host ticks (§12 rule 2, `0x005848C1`).
pub const PORTAL_HOSTILE_DELAY: u32 = 5000;

/// Stat ids the well heal reads (§11 rule 2; `combat/vitals.md`).
pub mod stat {
    pub const LIFE: u16 = 6;
    pub const MAX_LIFE: u16 = 7;
    pub const MANA: u16 = 8;
    pub const MAX_MANA: u16 = 9;
    pub const STAMINA: u16 = 10;
    pub const MAX_STAMINA: u16 = 11;
}

/// States the well heal removes (§11 rule 2), in order.
pub const WELL_REMOVED_STATES: [u16; 2] = [2, 1];

/// The door, well, portal and update-pass seams beyond [`ObjectWorld`].
/// Every default is the narrowest reading: nothing happens, or none.
#[allow(unused_variables)]
pub trait MiscWorld: ObjectWorld {
    /// Event 11: the creator `0x0056CF40` of the portal object `row`
    /// (`sim/units.md` §6.4).
    fn create_level_portal(&mut self, object: UnitId, row: u16) {}
    /// `0x00581AD0` rule 2 (§14): flag 0x400 sound, flag 0x100 hover,
    /// flags 2 bit 0 `0x00597890`, then `0x00571CD0`.
    fn update_extras(&mut self, object: UnitId) {}
    /// `0x0064D800` (§10, `sim/path-placement.md` §3): some cell of the
    /// object's footprint has collision bits in `mask`. Default: none.
    fn footprint_collides(&self, object: UnitId, mask: u16) -> bool {
        false
    }
    /// A vital stat of `unit` (§11 rule 2; `sim/stats.md`): 6 life, 8 mana,
    /// 10 stamina; the maxima 7, 9, 11 are read through the getters
    /// `0x00625D10`, `0x00625D60`, `0x00625DB0` (8.8 fixed). Default 0.
    fn vital_stat(&self, unit: UnitId, id: u16) -> u32 {
        0
    }
    /// Set a vital stat (6, 8 or 10) and send the client its stat update
    /// (§11 rule 2, `0x00585720`; message owned by `sim/stats.md`).
    fn set_vital_stat(&mut self, unit: UnitId, id: u16, value: u32) {}
    /// Remove the stat list of `state` from `unit` when present (§11 rule
    /// 2; `sim/stat-lists.md`). Returns whether one was removed. Default:
    /// none present.
    fn remove_state_list(&mut self, unit: UnitId, state: u16) -> bool {
        false
    }
    /// `0x00578C20` (§11 rule 2, `world/npc.md` §5 step 4): remove every
    /// curable state's stat list. Returns whether one was removed.
    fn cure_states(&mut self, unit: UnitId) -> bool {
        false
    }
    /// The well's pet heal (§11 rule 2, callback `0x005856A0` over the
    /// player's pets; pet order owned by `sim/pets.md`). Returns whether a
    /// pet changed. Default: no pets.
    fn well_heal_pets(&mut self, player: UnitId, object: UnitId) -> bool {
        false
    }
    /// `0x00535B10(game, player)` (§12 rule 1; D2MOO
    /// `D2GAME_IteratePlayers`). Default 0: the owner test is skipped.
    fn iterate_players(&self, player: UnitId) -> u32 {
        0
    }
    /// `0x0055B6C0(player, 0)` (§12 rule 2, `world/waypoints.md` §6.1):
    /// the host tick of the player's last hostility declaration. Default 0.
    fn hostile_time(&self, player: UnitId) -> u32 {
        0
    }
    /// §12 rule 3 (open question 7): the portal's party, quest-flag and
    /// owner checks and the travel itself. Returns the operate function's
    /// result, or `None` when the steps are not run (default: the spec
    /// does not state them yet).
    fn portal_travel(&mut self, object: UnitId, player: UnitId) -> Option<i32> {
        None
    }
}

/// Operate 8 `0x00581D40` (§10).
pub fn door<W: ObjectHost>(
    ctl: &mut ObjectControl,
    t: &ObjectTables,
    w: &mut W,
    op: &Operate,
) -> Result<i32, ObjectError> {
    let obj = op.object;
    // Rule 1 (host clock, edge case 9).
    // TODO(objects.md §10 rule 1): the sum is read as a wrapping u32 and
    // the compare as unsigned (`GetTickCount` is a DWORD).
    let now = w.host_tick();
    if now < ctl.get(obj)?.last_tick.wrapping_add(DOOR_DEBOUNCE) {
        return Ok(1);
    }
    // Rule 2.
    let mode = w.mode(obj);
    match mode {
        0 => open_door(ctl, t, w, obj, now)?,
        6 => {
            // TODO(objects.md §10 rule 2): read as the key test function
            // `0x0055F140` alone; the assassin exemption of §8.1 rule 2 is
            // in the chest code and not applied. No operator: no key.
            let pass = match op.operator {
                Some(p) => w.key_test(p),
                None => false,
            };
            if !pass {
                if let Some(p) = op.operator {
                    w.sound(p, sound::LOCKED, None, false);
                }
                return Ok(1);
            }
            open_door(ctl, t, w, obj, now)?;
        }
        2 | 5 => {
            if !w.footprint_collides(obj, DOOR_CLEAR_MASK) {
                w.stamp_footprint(obj);
                set_object_mode(ctl, t, w, obj, 0)?;
                ctl.get_mut(obj)?.last_tick = now;
            } else if w.footprint_collides(obj, DOOR_CORPSE_MASK) {
                set_object_mode(ctl, t, w, obj, 4)?;
                ctl.get_mut(obj)?.last_tick = now;
            } else if mode != 5 {
                set_object_mode(ctl, t, w, obj, 5)?;
                ctl.get_mut(obj)?.last_tick = now;
            }
        }
        // 1, 3, 4: nothing; > 6: return 1.
        _ => {}
    }
    // Rule 3.
    Ok(1)
}

/// §10 rule 2, mode 0: free the footprint, mode 2, store the tick.
fn open_door<W: ObjectHost>(
    ctl: &mut ObjectControl,
    t: &ObjectTables,
    w: &mut W,
    obj: UnitId,
    now: u32,
) -> Result<(), ObjectError> {
    w.free_footprint(obj);
    set_object_mode(ctl, t, w, obj, 2)?;
    ctl.get_mut(obj)?.last_tick = now;
    Ok(())
}

/// Operate 11 `0x005843D0` (§13).
pub fn torch<W: ObjectHost>(
    ctl: &mut ObjectControl,
    t: &ObjectTables,
    w: &mut W,
    op: &Operate,
) -> Result<i32, ObjectError> {
    let obj = op.object;
    match w.mode(obj) {
        0 => {
            set_object_mode(ctl, t, w, obj, 1)?;
            clear_selectable(w, obj);
        }
        1 | 2 => set_object_mode(ctl, t, w, obj, 0)?,
        _ => {}
    }
    Ok(1)
}

/// Operate 15 `0x00584870` (§12). `Ok(None)`: rules 1–2 passed and rule 3
/// (open question 7) was not run by the host ([`MiscWorld::portal_travel`]).
pub fn portal<W: ObjectHost>(
    ctl: &mut ObjectControl,
    _t: &ObjectTables,
    w: &mut W,
    op: &Operate,
) -> Result<Option<i32>, ObjectError> {
    let obj = op.object;
    // TODO(objects.md §12): the rules name a player operator; a monster or
    // no operator is read as refused (result 0).
    let Some(p) = op.operator else {
        return Ok(Some(0));
    };
    if !matches!(w.operator(p), Operator::Player(_)) {
        return Ok(Some(0));
    }
    // Rule 1: busy `0x00535060` (`items/inventory.md` §5.2: interact info,
    // cursor item, player data +0x4C).
    if w.interact_active(p) || w.cursor_item(p) || w.player_busy(p) {
        return Ok(Some(0));
    }
    if w.iterate_players(p) != 0 {
        // Owner: the timer argument's GUID (`0x00552B10`).
        let owner = ctl.get(obj)?.owner;
        if owner != Some(w.guid(p) as i32) {
            return Ok(Some(0));
        }
    }
    // Rule 2 (host clock, edge case 9).
    // TODO(objects.md §12 rule 2): wrapping u32 sum, unsigned compare.
    if w.host_tick() < w.hostile_time(p).wrapping_add(PORTAL_HOSTILE_DELAY) {
        w.sound(p, sound::PORTAL_REFUSED, None, false);
        return Ok(Some(0));
    }
    // Rule 3.
    // TODO(objects.md open question 7): the travel steps are not specified;
    // they are left to the host seam.
    Ok(w.portal_travel(obj, p))
}

/// The well mode rule (§11 rule 3, event 2): `c ≤ M` and `c mod (M / 2) =
/// 0` → mode 2 − c / `Parm2`.
fn well_mode(c: u32, parm2: u32) -> Result<Option<u8>, ObjectError> {
    let m = parm2.wrapping_mul(2);
    if c > m {
        return Ok(None);
    }
    // `Parm2` = 0 divides by zero in the original (a crash): fatal here.
    let half = m / 2;
    if half == 0 {
        return Err(ObjectError::WellCharges(c as u8));
    }
    if !c.is_multiple_of(half) {
        return Ok(None);
    }
    Ok(Some(2u32.wrapping_sub(c / parm2) as u8))
}

/// `0x00585720` (§11 rule 2): heal one vital stat. Returns whether it
/// changed.
fn heal_vital<W: ObjectHost>(w: &mut W, p: UnitId, id: u16, max_id: u16, parm1: u32) -> bool {
    let cur = w.vital_stat(p, id);
    let max = w.vital_stat(p, max_id);
    if cur >= max {
        return false;
    }
    let v = cur.wrapping_add(max.wrapping_mul(parm1) >> 8).min(max);
    w.set_vital_stat(p, id, v);
    // TODO(objects.md §11 rule 2): "changed" read as the value differing.
    v != cur
}

/// Operate 22 `0x005858A0` (§11). Returns 0 always (edge case 10).
pub fn well<W: ObjectHost>(
    ctl: &mut ObjectControl,
    t: &ObjectTables,
    w: &mut W,
    op: &Operate,
) -> Result<i32, ObjectError> {
    let obj = op.object;
    let o = t.object(op.class)?;
    // Rule 1.
    let c = ctl.get(obj)?.interact;
    if c == 0 {
        return Ok(0);
    }
    // Rule 2.
    let Some(p) = op.operator else {
        return Ok(0);
    };
    let mut used = false;
    if o.parm3 & 2 != 0 {
        used |= heal_vital(w, p, stat::LIFE, stat::MAX_LIFE, o.parm1);
    }
    if o.parm3 & 1 != 0 {
        used |= heal_vital(w, p, stat::MANA, stat::MAX_MANA, o.parm1);
    }
    used |= heal_vital(w, p, stat::STAMINA, stat::MAX_STAMINA, o.parm1);
    for s in WELL_REMOVED_STATES {
        used |= w.remove_state_list(p, s);
    }
    used |= w.cure_states(p);
    used |= w.well_heal_pets(p, obj);
    // Rule 3.
    if !used {
        return Ok(0);
    }
    let c = c - 1;
    if let Some(mode) = well_mode(u32::from(c), o.parm2)? {
        set_object_mode(ctl, t, w, obj, mode)?;
    }
    ctl.get_mut(obj)?.interact = c;
    let at = w.frame().wrapping_add(o.parm0 as i32).wrapping_add(1);
    w.schedule(obj, super::oevent::WELL_REFILL, at);
    Ok(0)
}

/// Event 2 `0x00581510` (§11): refill one charge.
pub fn well_refill<W: ObjectHost>(
    ctl: &mut ObjectControl,
    t: &ObjectTables,
    w: &mut W,
    obj: UnitId,
) -> Result<(), ObjectError> {
    let d = ctl.get(obj)?;
    let class = d.class;
    let c = u32::from(d.interact);
    let parm2 = t.object(class)?.parm2;
    if parm2 == 0 || c / parm2 >= 2 {
        return Err(ObjectError::WellCharges(c as u8));
    }
    let c = c + 1;
    if c > parm2.wrapping_mul(2) {
        return Err(ObjectError::WellCharges(c as u8));
    }
    if let Some(mode) = well_mode(c, parm2)? {
        // TODO(objects.md §11 event 2): the mode set is read as not
        // queueing; the explicit queue below does.
        set_mode(t, w, obj, class, mode, false)?;
    }
    ctl.get_mut(obj)?.interact = c as u8;
    w.queue_update(obj);
    let f = w.flags(obj);
    w.set_flags(obj, f | oflags::CHANGED);
    Ok(())
}
