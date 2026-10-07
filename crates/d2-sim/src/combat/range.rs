// Spec: specs/combat/hit.md §7.1–§7.3
//! Hostility `0x00554200`, in-melee-range `0x00622C40` and melee range
//! `0x00622870` as pure rules over [`RangeWorld`] (`combat/hit.md` §7).
//! A host implements [`RangeWorld`] from its unit data and answers the
//! [`super::CombatWorld`] seams (`hostile`, `in_melee_range`,
//! `melee_range`) with these functions.

use crate::units::UnitType;

/// Alignment values (`0x006259B0`): evil, neutral, good.
pub const ALIGN_EVIL: i32 = 0;
pub const ALIGN_GOOD: i32 = 2;
/// Monster `BaseId`s of the tentacle rule (§7.2 step 2).
pub const BASE_TENTACLE1: i32 = 258;
pub const BASE_TENTACLEHEAD1: i32 = 261;
/// The collision mask of the melee line test (§7.2 step 3).
pub const MELEE_LINE_MASK: u32 = 0x804;
/// Weapon class `2ht` (two-handed thrust/sword) read by §7.3 step 3.
pub const WEAPON_CLASS_2HT: i32 = 6;
/// The relation entry's hostile bit (§7.1 step 4).
pub const RELATION_HOSTILE: u32 = 8;
/// State 105 (`alignment`) and its stat 172 (§7.1).
pub const STATE_ALIGNMENT: u16 = 105;
pub const STAT_ALIGNMENT: u16 = 172;

/// The unit queries the rules read.
pub trait RangeWorld {
    type Unit: Copy + PartialEq;
    fn unit_type(&self, u: Self::Unit) -> UnitType;
    /// The owner (`0x0058F0D0`) of a unit, resolved.
    fn owner(&self, u: Self::Unit) -> Option<Self::Unit>;
    /// A missile's source-unit link (flag-ex +0xC8 bit 0x400; the unit of
    /// type +0x94 / GUID +0x98), resolved (`0x00552F60`); `None` without
    /// the flag or when the unit does not exist.
    fn missile_source(&self, u: Self::Unit) -> Option<Self::Unit>;
    fn guid(&self, u: Self::Unit) -> u32;
    /// The flags of the first relation entry with `guid` in `a`'s player
    /// data (`0x006221A0`); `None` without an entry.
    fn relation_flags(&self, a: Self::Unit, guid: u32) -> Option<u32>;
    /// Stat 172 of the unit's state-105 list (`0x006256B0`, `0x00625420`);
    /// 0 without a stat holder or without the list.
    fn alignment_stat(&self, u: Self::Unit) -> i32;
    /// A monster's `BaseId` (`0x00463860`).
    fn base_id(&self, u: Self::Unit) -> i32;
    /// Unit distance `0x00641530`.
    fn distance(&self, a: Self::Unit, b: Self::Unit) -> i32;
    /// `0x00622AA0(a, b, mask)` != 0: the collision line is blocked.
    fn line_blocked(&self, a: Self::Unit, b: Self::Unit, mask: u32) -> bool;
    /// The `rangeadder` (weapons +0x104) of the player's weapon pick
    /// `0x0063C9B0`; 0 without one.
    fn weapon_range_adder(&self, u: Self::Unit) -> i32;
    /// `MeleeRng` (monstats2 +0x0E) of the monster's class row; `None`
    /// without a row.
    fn melee_rng(&self, u: Self::Unit) -> Option<u8>;
    /// The weapon class in the unit's current mode
    /// (`0x0064F380(unit, inventory, &c, mode − 1, 1)`).
    fn weapon_class_now(&self, u: Self::Unit) -> i32;
}

/// Alignment `0x006259B0`: players and monsters read stat 172 of the
/// state-105 list; other unit types are 2 (good).
pub fn alignment<W: RangeWorld>(w: &W, u: Option<W::Unit>) -> i32 {
    let Some(u) = u else {
        return ALIGN_EVIL;
    };
    match w.unit_type(u) {
        UnitType::Player | UnitType::Monster => w.alignment_stat(u),
        _ => ALIGN_GOOD,
    }
}

/// Hostility `0x00554200` (§7.1): may `a` attack `d`.
pub fn hostile<W: RangeWorld>(w: &W, a: Option<W::Unit>, d: W::Unit) -> bool {
    // Step 1.
    let a2 = a.map(|a| match w.unit_type(a) {
        UnitType::Monster => w.owner(a).unwrap_or(a),
        UnitType::Missile => w.missile_source(a).unwrap_or(a),
        _ => a,
    });
    // Step 2: climb the owner chain. The original loops until a monster
    // has no owner or is its own owner; a cycle of owners would hang it,
    // so the walk is bounded.
    let mut d2 = d;
    for _ in 0..256 {
        if w.unit_type(d2) != UnitType::Monster {
            break;
        }
        match w.owner(d2) {
            Some(o) if o != d2 => d2 = o,
            _ => break,
        }
    }
    // Step 3.
    if a2 == Some(d2) {
        return false;
    }
    // Step 4.
    if let Some(a2) = a2 {
        if w.unit_type(a2) == UnitType::Player && w.unit_type(d2) == UnitType::Player {
            // The entry is looked up by the GUID of D (not D').
            return w
                .relation_flags(a2, w.guid(d))
                .is_some_and(|f| f & RELATION_HOSTILE != 0);
        }
    }
    // Step 5.
    let (aa, ad) = (alignment(w, a2), alignment(w, Some(d2)));
    !((aa == ALIGN_EVIL && ad == ALIGN_EVIL) || (aa == ALIGN_GOOD && ad == ALIGN_GOOD))
}

/// Melee range `0x00622870` (§7.3).
pub fn melee_range<W: RangeWorld>(w: &W, u: W::Unit) -> i32 {
    match w.unit_type(u) {
        UnitType::Player => w.weapon_range_adder(u),
        UnitType::Monster => match w.melee_rng(u) {
            None => 0,
            Some(255) => {
                if w.weapon_class_now(u) == WEAPON_CLASS_2HT {
                    2
                } else {
                    0
                }
            }
            Some(r) => i32::from(r),
        },
        _ => 0,
    }
}

/// In melee range `0x00622C40(a, b, extra)` (§7.2).
pub fn in_melee_range<W: RangeWorld>(
    w: &W,
    a: Option<W::Unit>,
    b: Option<W::Unit>,
    extra: i32,
) -> bool {
    let (Some(a), Some(b)) = (a, b) else {
        return false;
    };
    // Step 2.
    if w.unit_type(b) == UnitType::Monster
        && matches!(w.base_id(b), BASE_TENTACLE1 | BASE_TENTACLEHEAD1)
    {
        let d = w.distance(a, b);
        if melee_range(w, a) + 8 > d {
            return true;
        }
    }
    // Step 3.
    let r = melee_range(w, a).wrapping_add(extra).wrapping_add(1);
    let d = w.distance(a, b);
    if d <= 0 {
        return true;
    }
    if r < d {
        return false;
    }
    !w.line_blocked(a, b, MELEE_LINE_MASK)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[derive(Default)]
    struct W {
        ty: BTreeMap<usize, UnitType>,
        owner: BTreeMap<usize, usize>,
        msrc: BTreeMap<usize, usize>,
        rel: BTreeMap<(usize, u32), u32>,
        align: BTreeMap<usize, i32>,
        base: BTreeMap<usize, i32>,
        dist: i32,
        blocked: bool,
        adder: i32,
        rng: BTreeMap<usize, u8>,
        wclass: i32,
    }

    impl W {
        fn add(&mut self, u: usize, ty: UnitType, align: i32) {
            self.ty.insert(u, ty);
            self.align.insert(u, align);
        }
    }

    impl RangeWorld for W {
        type Unit = usize;
        fn unit_type(&self, u: usize) -> UnitType {
            self.ty[&u]
        }
        fn owner(&self, u: usize) -> Option<usize> {
            self.owner.get(&u).copied()
        }
        fn missile_source(&self, u: usize) -> Option<usize> {
            self.msrc.get(&u).copied()
        }
        fn guid(&self, u: usize) -> u32 {
            u as u32 + 100
        }
        fn relation_flags(&self, a: usize, guid: u32) -> Option<u32> {
            self.rel.get(&(a, guid)).copied()
        }
        fn alignment_stat(&self, u: usize) -> i32 {
            self.align.get(&u).copied().unwrap_or(0)
        }
        fn base_id(&self, u: usize) -> i32 {
            self.base.get(&u).copied().unwrap_or(0)
        }
        fn distance(&self, _: usize, _: usize) -> i32 {
            self.dist
        }
        fn line_blocked(&self, _: usize, _: usize, mask: u32) -> bool {
            assert_eq!(mask, 0x804);
            self.blocked
        }
        fn weapon_range_adder(&self, _: usize) -> i32 {
            self.adder
        }
        fn melee_rng(&self, u: usize) -> Option<u8> {
            self.rng.get(&u).copied()
        }
        fn weapon_class_now(&self, _: usize) -> i32 {
            self.wclass
        }
    }

    // Covers: specs/combat/hit.md §7.1 text, §7.1 r1, §7.1 r2, §7.1 r3, §7.1 r4, §7.1 r5
    #[test]
    fn hostility() {
        let mut w = W::default();
        w.add(0, UnitType::Player, 2); // player A
        w.add(1, UnitType::Player, 2); // player B
        w.add(2, UnitType::Monster, 0); // evil monster
        w.add(3, UnitType::Monster, 0); // evil monster
        w.add(4, UnitType::Monster, 2); // good monster
        w.add(5, UnitType::Monster, 1); // neutral monster
        w.add(6, UnitType::Missile, 0);
        w.add(7, UnitType::Object, 0);
        // 5: a player and an evil monster are on opposite sides; two evil
        // monsters and two good units are on the same side; a neutral unit
        // is hostile to everyone.
        assert!(hostile(&w, Some(0), 2));
        assert!(hostile(&w, Some(2), 0));
        assert!(!hostile(&w, Some(2), 3));
        assert!(!hostile(&w, Some(4), 0));
        assert!(hostile(&w, Some(5), 2));
        assert!(hostile(&w, Some(2), 5));
        assert!(!hostile(&w, Some(5), 5)); // 3: A' = D'
                                           // Other unit types count as good (2).
        assert!(!hostile(&w, Some(0), 7));
        assert!(hostile(&w, Some(2), 7));
        // 3: A' = D' -> 0; both none -> 0 (here: no attacker, evil target
        // has alignment 0 like "none").
        assert!(!hostile(&w, Some(2), 2));
        assert!(!hostile(&w, None, 2));
        // 1: a monster with an owner attacks as its owner; 2: a monster
        // defender with an owner is judged as the owner.
        w.owner.insert(4, 2); // good monster 4 owned by evil monster 2
        assert!(hostile(&w, Some(4), 0)); // as unit 2 vs player 0
        assert!(!hostile(&w, Some(4), 3)); // as unit 2 vs evil 3
        w.owner.insert(3, 0); // evil monster 3 is player 0's minion
        assert!(!hostile(&w, Some(0), 3)); // A' = D' = player 0
        assert!(hostile(&w, Some(2), 3)); // evil 2 vs player 0
                                          // 1: a missile attacks as its source unit.
        w.msrc.insert(6, 2);
        assert!(hostile(&w, Some(6), 0));
        // ...and the good monster 4 is judged as its owner 2: same unit.
        assert!(!hostile(&w, Some(6), 4));
        // 4: two players: only the relation entry's hostile bit; no entry
        // (single player) -> never hostile.
        assert!(!hostile(&w, Some(0), 1));
        w.rel.insert((0, 101), 0x7);
        assert!(!hostile(&w, Some(0), 1));
        w.rel.insert((0, 101), 0x8);
        assert!(hostile(&w, Some(0), 1));
        assert!(!hostile(&w, Some(1), 0));
    }

    // Covers: specs/combat/hit.md §7.3 r1, §7.3 r2, §7.3 r3, §7.3 r4
    #[test]
    fn melee_range_by_unit_type() {
        let mut w = W::default();
        w.add(0, UnitType::Player, 2);
        w.add(1, UnitType::Monster, 0);
        w.add(2, UnitType::Monster, 0);
        w.add(3, UnitType::Object, 0);
        // Player: the weapon's rangeadder.
        w.adder = 3;
        assert_eq!(melee_range(&w, 0), 3);
        // Monster: MeleeRng; 255 -> 2 with a `2ht` weapon, else 0; no row
        // -> 0.
        w.rng.insert(1, 4);
        w.rng.insert(2, 255);
        assert_eq!(melee_range(&w, 1), 4);
        assert_eq!(melee_range(&w, 2), 0);
        w.wclass = WEAPON_CLASS_2HT;
        assert_eq!(melee_range(&w, 2), 2);
        w.ty.insert(9, UnitType::Monster);
        assert_eq!(melee_range(&w, 9), 0);
        // Other unit types: 0.
        assert_eq!(melee_range(&w, 3), 0);
    }

    // Covers: specs/combat/hit.md §7.2 r1, §7.2 r2, §7.2 r3
    #[test]
    fn in_range() {
        let mut w = W::default();
        w.add(0, UnitType::Player, 2);
        w.add(1, UnitType::Monster, 0);
        w.add(2, UnitType::Monster, 0);
        w.adder = 1;
        // 1: a unit missing -> 0.
        assert!(!in_melee_range(&w, None, Some(1), 1));
        assert!(!in_melee_range(&w, Some(0), None, 1));
        // 3: r = range 1 + extra 1 + 1 = 3. d <= 0 -> 1 (even blocked).
        w.blocked = true;
        w.dist = 0;
        assert!(in_melee_range(&w, Some(0), Some(1), 1));
        w.dist = -4;
        assert!(in_melee_range(&w, Some(0), Some(1), 1));
        // r < d -> 0; r >= d needs a clear line.
        w.dist = 4;
        assert!(!in_melee_range(&w, Some(0), Some(1), 1));
        w.dist = 3;
        assert!(!in_melee_range(&w, Some(0), Some(1), 1)); // blocked
        w.blocked = false;
        assert!(in_melee_range(&w, Some(0), Some(1), 1));
        // 2: a tentacle target is in range when melee_range + 8 > d, with
        // no line test.
        w.base.insert(2, 258);
        w.blocked = true;
        w.dist = 8;
        assert!(in_melee_range(&w, Some(0), Some(2), 1)); // 1 + 8 > 8
        w.dist = 9;
        assert!(!in_melee_range(&w, Some(0), Some(2), 1)); // falls through
        w.base.insert(2, 261);
        w.dist = 8;
        assert!(in_melee_range(&w, Some(0), Some(2), 1));
        // Any other base id takes the plain rule.
        w.base.insert(2, 259);
        assert!(!in_melee_range(&w, Some(0), Some(2), 1));
    }
}
