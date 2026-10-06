// Spec: specs/world/objects.md §9
//! Shrines: operate 2 (§9.1), the effects (§9.2, §9.3), events 5 and 6.

use crate::units::UnitId;

use super::{ObjectControl, ObjectError, ObjectHost, ObjectTables, ObjectWorld, Operate};

#[cfg(test)]
mod tests;

/// The shrine seams beyond [`ObjectWorld`]. Every default is the narrowest
/// reading: nothing happens, or none.
#[allow(unused_variables)]
pub trait ShrineWorld: ObjectWorld {}

/// Operate 2 `0x00583C70` (§9.1).
pub fn operate<W: ObjectHost>(
    ctl: &mut ObjectControl,
    t: &ObjectTables,
    w: &mut W,
    op: &Operate,
) -> Result<i32, ObjectError> {
    let _ = (ctl, t, w, op);
    Ok(0)
}

/// Event 5 `0x005814D0` (§9.1).
pub fn reset_event<W: ObjectHost>(
    ctl: &mut ObjectControl,
    t: &ObjectTables,
    w: &mut W,
    obj: UnitId,
) -> Result<(), ObjectError> {
    let _ = (ctl, t, w, obj);
    Ok(())
}

/// Event 6 `0x00581620` (§9.1).
pub fn hover_event<W: ObjectHost>(
    ctl: &mut ObjectControl,
    t: &ObjectTables,
    w: &mut W,
    obj: UnitId,
) -> Result<(), ObjectError> {
    let _ = (ctl, t, w, obj);
    Ok(())
}
