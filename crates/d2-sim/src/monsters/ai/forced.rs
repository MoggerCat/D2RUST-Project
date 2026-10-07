// Spec: specs/monsters/ai.md §5.1
//! Forced targets `0x005DD610(game, unit, a, s, &target, &distance)`: the
//! monster-data override (kind +0x38, GUID +0x34) that terror, confuse and
//! skill callbacks set. The world is reached through [`ForcedWorld`].

use crate::units::{UnitId, UnitType};

/// What the forced-target check reads and writes.
pub trait ForcedWorld {
    /// Monster data override (kind, GUID); `None` for a non-monster.
    fn override_of(&self, unit: UnitId) -> Option<(u32, u32)>;
    /// `0x00573120`: kind, GUID := 0.
    fn clear_override(&mut self, unit: UnitId);
    /// `0x00552F60(type, GUID)`.
    fn find_by_guid(&self, ty: UnitType, guid: u32) -> Option<UnitId>;
    /// `0x005DC380`: full-size distance from `from` to `to` (§6).
    fn full_distance(&self, from: UnitId, to: UnitId) -> i32;
    /// `0x00622AA0(a, b, 4)` hits.
    fn line_blocked(&self, a: UnitId, b: UnitId) -> bool;
    /// `0x006259B0`.
    fn alignment(&self, unit: UnitId) -> u8;
    /// `0x005543B0(unit, value, 1)`.
    fn set_alignment(&mut self, unit: UnitId, value: u8);
    /// One raw step of the unit seed, `& 1` (`0x00472210(seed, 2)`).
    fn seed_bit(&mut self, unit: UnitId) -> u32;
    /// Room scan 5 (`s = 0`, context with LOS flag `a`, radius 35) or scan
    /// 6 (`s ≠ 0`) of §5.4: the best unit and its distance.
    fn scan(&mut self, unit: UnitId, scan: u8, a: bool) -> Option<(UnitId, i32)>;
    fn unit_type(&self, unit: UnitId) -> Option<UnitType>;
    /// `0x005541B0`.
    fn is_dead(&self, unit: UnitId) -> bool;
}

/// The temporary alignment of kind 3 (confuse) for alignment `a` and the
/// drawn bit `r` (§5.1 rule 3).
pub fn confused_alignment(a: u8, r: u32) -> u8 {
    match (a, r != 0) {
        (1, true) => 2,
        (1, false) => 0,
        (0, true) => 2,
        (2, true) => 0,
        _ => a,
    }
}

/// §5.1 rules 1–4: the forced target and its distance, or none.
pub fn forced_target<W: ForcedWorld + ?Sized>(
    w: &mut W,
    unit: UnitId,
    a: bool,
    s: bool,
) -> Option<(UnitId, i32)> {
    // 1.
    let (k, g) = w.override_of(unit)?;
    if k == 0 {
        return None;
    }
    let found = match k {
        // 2.
        1 | 2 | 4 => {
            let ty = match k {
                1 => UnitType::Player,
                2 => UnitType::Monster,
                _ => UnitType::Missile,
            };
            let Some(u) = w.find_by_guid(ty, g) else {
                w.clear_override(unit);
                return None;
            };
            let d = w.full_distance(u, unit);
            if s && w.line_blocked(u, unit) {
                w.clear_override(unit);
                return None;
            }
            Some((u, d))
        }
        // 3.
        3 => {
            let original = w.alignment(unit);
            let r = w.seed_bit(unit);
            w.set_alignment(unit, confused_alignment(original, r));
            let best = w.scan(unit, if s { 6 } else { 5 }, a);
            w.set_alignment(unit, original);
            best
        }
        // PROVISIONAL (ai.md §5.1; REC-80): kinds ≥ 5 have no jump-table
        // case; the setter never stores them (k < 5), so none is found.
        _ => None,
    };
    // 4.
    let accept = found.filter(|&(u, _)| match w.unit_type(u) {
        Some(UnitType::Player | UnitType::Monster) => !w.is_dead(u),
        Some(UnitType::Missile) => true,
        _ => false,
    });
    if accept.is_none() {
        w.clear_override(unit);
    }
    accept
}
