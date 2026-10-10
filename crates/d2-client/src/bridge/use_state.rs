// Spec: specs/ui/controls.md (§6 r9.1), specs/skills/use.md (§2), specs/skills/levels.md (§4), specs/client/model.md (§8 r7)
//! The client's use state of the local player's click (`0x004D9FC0` =
//! `skills/use.md` §2 `0x00647960` over the model) and the client
//! start's `cltstfunc` result (`client/model.md` §8 r7 step 5), which
//! decide whether a skill click sends anything (`ui/controls.md` §6 r9.1;
//! `skills/sequences.md` local player rule 2: the C→S message leaves only
//! when the mode request returned non-zero).

use super::msg::skills::level_with_bonuses;
use super::skills::{SkillEntry, NATIVE};
use super::world::{ClientWorld, ModelInputs, SkillRow, UnitKey, ITEM, MONSTER};

/// The use state codes (`skills/use.md` §2).
pub mod code {
    pub const USABLE: u32 = 0;
    pub const NO_MANA: u32 = 1;
    pub const SHAPE: u32 = 4;
    pub const DISABLED: u32 = 3;
    pub const PASSIVE: u32 = 5;
    pub const AURA: u32 = 6;
    pub const NO_LEVEL: u32 = 7;
}

/// State 114 `blood_mana` (`skills/levels.md` §4).
const STATE_BLOOD_MANA: u8 = 114;
/// The were-forms' `srvdofunc` (`skills/levels.md` §4).
const SRVDOFUNC_WERE: i16 = 116;
/// Stats `hitpoints` (6) and `mana` (8).
const STAT_LIFE: u16 = 6;
const STAT_MANA: u16 = 8;
/// Flag-ex (+0xC8) bit 0x8: the shapeshift test `0x0063A400`
/// (`combat/vitals.md` DT start step 1).
const FLAG_EX_SHAPESHIFT: u32 = 0x8;

/// `use_state(P, entry)` (`skills/use.md` §2, first failure wins) of
/// `key`'s entry `e`: (1) no record or `InGame` clear → 3; (2) level 0
/// → 7; (3) `aura` → 6; (4) `passive` → 5; (6) `can_afford` fails → 1
/// (`skills/levels.md` §4).
// PROVISIONAL (skills/use.md §2 tests 5, 8-10; REC-1640): the skill item
// test, start stat, charges and cooldown tests (and the local
// player's code 8 of `ui/controls.md` §6 r9.1) read as passing: the model
// holds no client cooldown or item-skill facts yet; settled by a 1.14d
// check casting a skill on cooldown or without its item.
pub fn use_state(w: &ClientWorld, inputs: &ModelInputs, key: UnitKey, e: &SkillEntry) -> u32 {
    let t = &inputs.tables;
    let Some(r) = t.skills.get(usize::from(e.skill)) else {
        return code::DISABLED;
    };
    if !r.ingame {
        return code::DISABLED;
    }
    let level = level_with_bonuses(w, key, &t.skills, &t.skilldesc, e);
    if level == 0 {
        return code::NO_LEVEL;
    }
    if r.aura {
        return code::AURA;
    }
    if r.flags & crate::controls::click::skill_flag::PASSIVE != 0 {
        return code::PASSIVE;
    }
    if !can_afford(w, key, r, e, level) {
        return code::NO_MANA;
    }
    if !shape_ok(w, inputs, key, r) {
        return code::SHAPE;
    }
    code::USABLE
}

/// The shape test `0x00644060` (`skills/use.md` §2 test 7): `restrict` 0
/// passes only outside a shapeshift form (no state with the `states.txt`
/// `restrict` flag, `0x0063A510`), 1 always, 2 only in a form that has
/// one of the row's `State1`…`State3` (a negative id ends the list).
fn shape_ok(w: &ClientWorld, inputs: &ModelInputs, key: UnitKey, r: &SkillRow) -> bool {
    let Some(u) = w.units.get(&key) else {
        return false;
    };
    let in_form = || {
        u.states.iter().any(|&s| {
            inputs
                .tables
                .states
                .get(usize::from(s))
                .is_some_and(|row| row.restrict)
        })
    };
    match r.restrict {
        0 => !in_form(),
        2 => {
            in_form()
                && r.shape_states
                    .iter()
                    .take_while(|&&s| s >= 0)
                    .any(|&s| u.states.contains(&(s as u8)))
        }
        _ => true,
    }
}

/// `can_afford(unit, skill)` `0x00647540` (`skills/levels.md` §4): a
/// charge entry needs charges; else c = (mana + lvlmana · max(L − 1, 0))
/// << manashift against life (state 114) or mana; a were-form skill while
/// shapeshifted always passes.
fn can_afford(w: &ClientWorld, key: UnitKey, r: &SkillRow, e: &SkillEntry, level: i32) -> bool {
    if e.owner != NATIVE {
        return e.charges > 0;
    }
    let n = (level - 1).max(0);
    let c = i32::from(r.mana as i16)
        .wrapping_add(i32::from(r.lvlmana as i16).wrapping_mul(n))
        .wrapping_shl(u32::from(r.manashift as u8));
    let Some(u) = w.units.get(&key) else {
        return false;
    };
    if u.states.contains(&STATE_BLOOD_MANA) {
        w.total(key, STAT_LIFE, 0) >= c
    } else if r.srvdofunc == SRVDOFUNC_WERE && u.flag_ex & FLAG_EX_SHAPESHIFT != 0 {
        true
    } else {
        w.total(key, STAT_MANA, 0) >= c
    }
}

/// The `cltstfunc` result of the client skill start (`client/model.md`
/// §8 r7 step 5) for skill `skill` with target `target` (the unit of a
/// code 0x16 request; none for 0x15, whose start clears the target):
/// false = the start returns 0 (current := none, mode set 1, and the
/// click sends nothing).
// PROVISIONAL (client/model.md §8 r7 step 5; REC-1641): the `cltstfunc`
// bodies (table `0x00727A90`) are not specified. Read from the 1.14d skill
// checks (q-chk-skills-4cls, Wine 2026-10-09: a right click on open ground
// next to a live cow sends no C->S for Raise Skeleton / Skeletal Mage
// (20), Corpse / Poison Explosion (21), Iron Golem (23), Revive (24),
// Psychic Hammer and Dragon Flight (5), and sends 0x0C for Bone Prison
// (22), the curses (18), Teeth (19) and the rest): 5 needs a target
// unit, 20 / 21 / 24 a dead monster, 23 an item; every other function
// passes. Settled by the table bodies (spec-writing session) and a
// 1.14d check of each on its target kind.
pub fn client_start_passes(
    w: &ClientWorld,
    inputs: &ModelInputs,
    caster: UnitKey,
    skill: u16,
    target: Option<UnitKey>,
) -> bool {
    let Some(r) = inputs.tables.skills.get(usize::from(skill)) else {
        return true;
    };
    let unit = target.and_then(|k| w.units.get(&k));
    match r.cltstfunc {
        // `0x004F2010` (1.14d export read 2026-10-09): a target T, U's
        // and T's rooms not in town (`0x0061AB00`), T a player or monster
        // and hostile (`0x00465C60`).
        5 => unit.is_some_and(|u| {
            u.key.unit_type <= MONSTER
                && !super::modes::in_town(w, caster)
                && !super::modes::in_town(w, u.key)
                && super::combat::hostile(w, inputs, u.key)
        }),
        20 | 21 | 24 => unit.is_some_and(|u| u.key.unit_type == MONSTER && u.is_dead()),
        23 => unit.is_some_and(|u| u.key.unit_type == ITEM),
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bridge::skills::SkillList;
    use crate::bridge::world::{ClientUnit, PLAYER};

    fn world(mana: i32) -> (ClientWorld, ModelInputs, UnitKey) {
        let mut inputs = ModelInputs::default();
        // Row 85 BloodGolem-like: mana 25, lvlmana 4, shift 8; row 70
        // Raise Skeleton-like: mana 6, lvlmana 1.
        inputs.tables.skills = vec![SkillRow::default(); 100];
        inputs.tables.skills[85] = SkillRow {
            ingame: true,
            mana: 25,
            lvlmana: 4,
            manashift: 8,
            ..SkillRow::default()
        };
        inputs.tables.skills[70] = SkillRow {
            ingame: true,
            mana: 6,
            lvlmana: 1,
            manashift: 8,
            cltstfunc: 20,
            ..SkillRow::default()
        };
        let key = UnitKey::new(PLAYER, 1);
        let mut p = ClientUnit::new(key);
        p.stats.insert(STAT_MANA, mana);
        p.skills = Some(SkillList::default());
        let mut w = ClientWorld::default();
        w.units.insert(key, p);
        (w, inputs, key)
    }

    const PLAYER_KEY: UnitKey = UnitKey::new(PLAYER, 1);

    fn entry(skill: u16, base: i32) -> SkillEntry {
        SkillEntry {
            skill,
            base,
            owner: NATIVE,
            ..SkillEntry::default()
        }
    }

    // Covers: specs/skills/use.md §2; specs/skills/levels.md §4
    /// nec-bloodgolem (1.14d, Wine 2026-10-09): level 20 costs (25 + 4 ·
    /// 19) << 8 = 25856; the necromancer's 21248 mana refuses it (no C→S
    /// at the click), Raise Skeleton's (6 + 19) << 8 = 6400 passes.
    #[test]
    fn mana_below_the_level_cost_is_use_state_1() {
        let (w, i, k) = world(21248);
        assert_eq!(use_state(&w, &i, k, &entry(85, 20)), code::NO_MANA);
        assert_eq!(use_state(&w, &i, k, &entry(70, 20)), code::USABLE);
        let (w, i, k) = world(25856);
        assert_eq!(use_state(&w, &i, k, &entry(85, 20)), code::USABLE);
        // Level 0 and a record without `InGame` come first.
        assert_eq!(use_state(&w, &i, k, &entry(85, 0)), code::NO_LEVEL);
        assert_eq!(use_state(&w, &i, k, &entry(3, 1)), code::DISABLED);
        assert_eq!(use_state(&w, &i, k, &entry(500, 1)), code::DISABLED);
    }

    // Covers: specs/client/model.md §8 r7
    #[test]
    fn a_corpse_start_without_a_dead_monster_fails() {
        let (mut w, i, _) = world(0);
        assert!(!client_start_passes(&w, &i, PLAYER_KEY, 70, None));
        let m = UnitKey::new(MONSTER, 9);
        let mut cow = ClientUnit::new(m);
        cow.mode = 1;
        w.units.insert(m, cow);
        assert!(!client_start_passes(&w, &i, PLAYER_KEY, 70, Some(m)));
        w.units.get_mut(&m).expect("cow").mode = 12;
        assert!(client_start_passes(&w, &i, PLAYER_KEY, 70, Some(m)));
        // A function without a rule passes.
        assert!(client_start_passes(&w, &i, PLAYER_KEY, 85, None));
    }

    // Covers: specs/skills/use.md §2
    /// gen-skill-dru-243 (1.14d): Shock Wave (`restrict` 2, `State1`
    /// bear) is refused outside bear form; `restrict` 0 refuses in a form.
    #[test]
    fn shape_restriction_is_use_state_4() {
        let (mut w, mut i, k) = world(100_000);
        i.tables.states = vec![Default::default(); 150];
        i.tables.states[140].restrict = true; // a form
        i.tables.states[141].restrict = true; // the bear
        i.tables.skills[85].restrict = 2;
        i.tables.skills[85].shape_states = [141, -1, 0];
        i.tables.skills[70].restrict = 0;
        assert_eq!(use_state(&w, &i, k, &entry(85, 20)), code::SHAPE);
        assert_eq!(use_state(&w, &i, k, &entry(70, 20)), code::USABLE);
        // In another form: still refused; the list stops at a negative id.
        w.units.get_mut(&k).expect("p").states.insert(140);
        assert_eq!(use_state(&w, &i, k, &entry(85, 20)), code::SHAPE);
        assert_eq!(use_state(&w, &i, k, &entry(70, 20)), code::SHAPE);
        w.units.get_mut(&k).expect("p").states.insert(141);
        assert_eq!(use_state(&w, &i, k, &entry(85, 20)), code::USABLE);
        // `restrict` 1: any form.
        i.tables.skills[85].restrict = 1;
        assert_eq!(use_state(&w, &i, k, &entry(85, 20)), code::USABLE);
    }
}
