// Spec: specs/client/msg-skills.md §2 rule 4 (`0x00646D60`), specs/client/stat-lists.md §4 rule 3 (`0x00643620`), specs/skills/levels.md §7.2
//! The passive-state refresh `0x00646D60(unit, skill)`: the stat list of a
//! passive skill's state (Critical Strike, Dodge, Avoid, Evade, Penetrate,
//! the masteries ...) holds the skill's `passivestat1…5` set to
//! `eval(passivecalc_i, skill, L)`, plus the markers 350 (skill) and
//! 351 (level).
//!
//! Layered stats (`passiveitype` > 0, the weapon masteries) are set on
//! the layer `passiveitype` (the itemtypes row), as `0x00646D60` passes
//! it to `0x00627150` (1.14d `gen-skill-bar-126`: the 0xA8 mastery lists
//! carry the item type as the stat's param).

use super::helpers::{eval, rec, s16, stat_ok, state_ok};
use super::BodyWorld;
use crate::skills::levels::{highest_entry, skill_level};
use crate::skills::SkillTables;

/// Stat 350: the passive list's skill marker.
pub const PASSIVE_SKILL: i32 = 350;
/// Stat 351: the passive list's level marker.
pub const PASSIVE_LEVEL: i32 = 351;

/// `0x00646D60(unit, skill)` (`client/msg-skills.md` §2 rule 4).
pub fn refresh<W: BodyWorld>(w: &mut W, t: &SkillTables, u: W::Unit, skill: i32) {
    let Some(r) = rec(t, skill) else {
        return;
    };
    let p = s16(r.passivestate);
    if p <= 0 || !state_ok(w, p) {
        return;
    }
    let entry = highest_entry(&w.skill_list(u), skill);
    let aura = s16(r.aurastate);
    let level = entry
        .filter(|_| aura <= 0 || !w.has_state(u, aura as u16))
        .map_or(0, |e| skill_level(w, t, Some(u), Some(&e), true));
    let found = w.state_list(u, p);
    // The state list of `0x00643620`: L = 0 detaches and frees it.
    if level <= 0 {
        if let Some(l) = found {
            w.detach_free(u, l);
        }
        return;
    }
    let l = match found {
        Some(l) => l,
        None => {
            let Some(l) = w.alloc_list(0, 0, Some(u)) else {
                return;
            };
            w.set_list_state(l, p);
            w.attach(u, l);
            l
        }
    };
    if w.list_get(l, PASSIVE_LEVEL) == level {
        return;
    }
    // The layer is `passiveitype` when it is > 0, else 0.
    let layer = u16::try_from(s16(r.passiveitype)).unwrap_or(0);
    let pairs = [
        (r.passivestat1, r.passivecalc1),
        (r.passivestat2, r.passivecalc2),
        (r.passivestat3, r.passivecalc3),
        (r.passivestat4, r.passivecalc4),
        (r.passivestat5, r.passivecalc5),
    ];
    for (stat, calc) in pairs {
        let s = s16(stat);
        if !stat_ok(t, s) {
            break;
        }
        let v = eval(w, t, u, calc, skill, level);
        w.list_set_layer(l, s, v, layer);
    }
    w.list_set(l, PASSIVE_SKILL, skill);
    w.list_set(l, PASSIVE_LEVEL, level);
    w.queue_update(u);
}
