// Spec: specs/world/objects.md §8
//! Chests and breakables (§8.1, §8.2) and traps (§8.3).

use crate::units::UnitId;

use super::{ObjectControl, ObjectError, ObjectHost, ObjectTables, ObjectWorld, Operate};

#[cfg(test)]
mod tests;

/// The chest seams beyond [`ObjectWorld`]. Every default is the narrowest
/// reading: nothing happens, or none.
#[allow(unused_variables)]
pub trait ChestWorld: ObjectWorld {}

/// Operate 1, 3, 4, 5, 7, 14, 68 (§8.1, §8.2).
pub fn operate<W: ObjectHost>(
    ctl: &mut ObjectControl,
    t: &ObjectTables,
    w: &mut W,
    op: &Operate,
) -> Result<i32, ObjectError> {
    let _ = (ctl, t, w, op);
    Ok(1)
}

/// Event 4 `0x005817A0` (§8.3).
pub fn trap_event<W: ObjectHost>(
    ctl: &mut ObjectControl,
    t: &ObjectTables,
    w: &mut W,
    obj: UnitId,
) -> Result<(), ObjectError> {
    let _ = (ctl, t, w, obj);
    Ok(())
}
