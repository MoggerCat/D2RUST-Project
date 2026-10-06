// Spec: specs/world/objects.md §10–§13, §14 rule 2
//! Doors (§10), wells (§11, event 2), portals (§12) and torches (§13).

use crate::units::UnitId;

use super::{ObjectControl, ObjectError, ObjectHost, ObjectTables, ObjectWorld, Operate};

#[cfg(test)]
mod tests;

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
}

/// Operate 8 `0x00581D40` (§10).
pub fn door<W: ObjectHost>(
    ctl: &mut ObjectControl,
    t: &ObjectTables,
    w: &mut W,
    op: &Operate,
) -> Result<i32, ObjectError> {
    let _ = (ctl, t, w, op);
    Ok(1)
}

/// Operate 11 `0x005843D0` (§13).
pub fn torch<W: ObjectHost>(
    ctl: &mut ObjectControl,
    t: &ObjectTables,
    w: &mut W,
    op: &Operate,
) -> Result<i32, ObjectError> {
    let _ = (ctl, t, w, op);
    Ok(1)
}

/// Operate 15 `0x00584870` (§12).
pub fn portal<W: ObjectHost>(
    ctl: &mut ObjectControl,
    t: &ObjectTables,
    w: &mut W,
    op: &Operate,
) -> Result<i32, ObjectError> {
    let _ = (ctl, t, w, op);
    Ok(0)
}

/// Operate 22 `0x005858A0` (§11).
pub fn well<W: ObjectHost>(
    ctl: &mut ObjectControl,
    t: &ObjectTables,
    w: &mut W,
    op: &Operate,
) -> Result<i32, ObjectError> {
    let _ = (ctl, t, w, op);
    Ok(0)
}

/// Event 2 `0x00581510` (§11).
pub fn well_refill<W: ObjectHost>(
    ctl: &mut ObjectControl,
    t: &ObjectTables,
    w: &mut W,
    obj: UnitId,
) -> Result<(), ObjectError> {
    let _ = (ctl, t, w, obj);
    Ok(())
}
