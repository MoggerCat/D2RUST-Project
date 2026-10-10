// Spec: specs/skills/use.md §5.2 (players; monsters `0x005A7670`), §7; specs/missiles/missiles.md §R2.2; specs/combat/damage.md §5.1; specs/sim/stat-lists.md §10.2, §10.3; specs/sim/units.md §4.6 r7, r10, r13, §5, §6.1; specs/skills/bodies-2.md §2.1 (mode damage of the mode set)
//! The skill timer events of the unit dispatch on the skill use
//! pipeline: event 5 (active state), 8 (periodic skills and auras) and 9
//! (item auras) reach [`crate::skills::use_`] through [`UseView`].
//!
//! The unit dispatch (`units.md` §5, `stat-lists.md` §10.2, §10.3) does
//! the checks it owns (skill id range, `srvactivefunc` < 191, item-aura
//! level from stat 151, the type-9 cancel) and calls the action hooks,
//! which hand the event to [`Pending::skill_event`]. A seam value that
//! also implements [`UseRest`] routes it here:
//!
//! ```text
//! fn skill_event(h: &mut ActionHooks<Self>, sim: &mut Sim<'_>, ev: SkillEvent) {
//!     d2_sim::wiring::interaction::skill_events::route(h, sim, ev)
//! }
//! ```
//!
//! Player event 0 in an attack, cast or skill mode (the action frame
//! `0x00580460`, `units.md` §4.5) reaches
//! [`crate::skills::use_::attack_frame_event`] the same way, through
//! [`Pending::action_frame`] and [`action_frame`].
//!
//! Event 14 (callback `0x00554570`) is not routed: `use.md` §6 states it
//! is never scheduled in 1.14d and its body is not specified.

use crate::combat::apply_melee;
use crate::skills::levels::skill_level;
use crate::skills::use_::bodies::b4_mon::monster_mode_missile;
use crate::skills::use_::bodies::{melee_setup, mode_damage, BodyWorld};
use crate::skills::use_::{
    active_state_event, attack_frame_event, do_skill, item_aura_event, periodic_event, start,
    UseWorld, FLAG_MISSILE_FIRED, SKILL_ARRIVED, SKILL_MOVING,
};
use crate::skills::SkillUnits;
use crate::units::hooks::Sim;
use crate::units::UnitId;
use crate::wiring::action::combat::CombatView;
use crate::wiring::action::{ActionHooks, Pending, SkillEvent, View};

use super::{UseRest, UseView};

/// Runs one skill timer event on the skill use pipeline.
pub fn route<X: Pending + UseRest>(h: &mut ActionHooks<X>, sim: &mut Sim<'_>, ev: SkillEvent) {
    let t = h.tables.clone();
    let mut w = UseView {
        cv: CombatView {
            game: sim.game,
            v: View::of(sim.units, sim.stats, sim.data, h),
        },
    };
    match ev {
        // `stat-lists.md` §10.2: f(game, unit, skill, a2).
        // TODO(stat-lists.md §10.2): a2 is read as the do function's
        // level argument (not stated).
        SkillEvent::ActiveState {
            unit,
            f,
            skill,
            arg2,
        } => {
            active_state_event(&mut w, unit, f, skill as i32, arg2 as i32);
        }
        // `use.md` §7: arg −1 (the aura form) or a skill id; the level of
        // the second form comes from stat 351, not from a2.
        SkillEvent::Periodic { unit, arg1, .. } => {
            periodic_event(&mut w, &t.skills, unit, arg1 as i32);
        }
        // `stat-lists.md` §10.3: `0x0056F7F0`(game, unit, skill, l, 1, 1, 0).
        //
        // TODO(stat-lists.md §10.3): the second call `0x0056CE70`(game,
        // unit, a1, skill, l, 0) is not specified (no body in `use.md`);
        // it is not run, so nothing reschedules the type-9 event.
        SkillEvent::ItemAura {
            unit, skill, level, ..
        } => {
            item_aura_event(&mut w, &t.skills, unit, skill as i32, level);
        }
    }
}

/// The player action frame `0x00580460` (`use.md` §5.2) on the skill use
/// pipeline: the used skill's do function on an action event (a1 1 or
/// 2); returns 1, or 2 when the unit died.
pub fn action_frame<X: Pending + UseRest>(
    h: &mut ActionHooks<X>,
    sim: &mut Sim<'_>,
    unit: UnitId,
    a1: u32,
    a2: u32,
) -> u32 {
    let t = h.tables.clone();
    let mut w = UseView {
        cv: CombatView {
            game: sim.game,
            v: View::of(sim.units, sim.stats, sim.data, h),
        },
    };
    attack_frame_event(&mut w, &t.skills, unit, a1 as i32, a2 as i32) as u32
}

/// The skill start `0x0056FAF0` (`use.md` §5.3) of a monster's attack /
/// skill and sequence starts (`units.md` §4.6 rules 7, 10) on the skill
/// use pipeline; its result.
pub fn monster_skill_start<X: Pending + UseRest>(
    h: &mut ActionHooks<X>,
    sim: &mut Sim<'_>,
    unit: UnitId,
) -> i32 {
    let t = h.tables.clone();
    let mut w = UseView {
        cv: CombatView {
            game: sim.game,
            v: View::of(sim.units, sim.stats, sim.data, h),
        },
    };
    start(&mut w, &t.skills, unit)
}

/// The right skill's aura part of the save load's assign (`0x005701B0`,
/// `use.md` §7 "0x3C SelectSkill"): the loaded right skill is an `aura`
/// skill → `immediate` (Might, Resist Fire, ...) runs the do core once,
/// else its aura state is switched on (stats 350 / 351); then the aura
/// form of `schedule_periodic` (q-fix-pt-right-aura). The load selects
/// the hand before the unit has a room, so the first periodic do runs on
/// the first tick after game entry.
pub fn assign_right_aura<X: Pending + UseRest>(
    h: &mut ActionHooks<X>,
    sim: &mut Sim<'_>,
    unit: UnitId,
) {
    let t = h.tables.clone();
    let mut w = UseView {
        cv: CombatView {
            game: sim.game,
            v: View::of(sim.units, sim.stats, sim.data, h),
        },
    };
    let Some(e) = w.right_skill(unit) else {
        return;
    };
    let Some(r) = t.skills.skill(e.skill).filter(|r| r.aura) else {
        return;
    };
    let l = skill_level(&w, &t.skills, Some(unit), Some(&e), true);
    if l == 0 {
        return;
    }
    if r.immediate {
        crate::skills::use_::do_core(&mut w, &t.skills, unit, e.skill, l, true, false, false);
    } else {
        w.set_aura_state(unit, r.aurastate, e.skill, l);
    }
    crate::skills::use_::schedule_periodic(&mut w, &t.skills, unit, e.skill, l, true);
}

/// The skill part of the monster sequence event 0 `0x005A8670`
/// (`units.md` §4.6 rule 13) on the skill use pipeline: E := the used
/// skill, f := its E flags (`0x006446A0`; 0 without a used skill), "do
/// left" := 1. f bit 0 (a moving skill): the target check and step
/// (`0x00553490`, `0x00554CA0`; the path provider's step when it is on);
/// result 2 → E flags := f | 2 (`0x00644660`), the do `0x0056FC50`, do
/// left := 0. Then by the frame code (unit byte +0x4E): 4 → the do; else
/// do left, unit flag 0x40 clear and code 1 or 2 → the do. The animation
/// refresh that follows is the caller's.
pub fn monster_sequence_frame<X: Pending + UseRest>(
    h: &mut ActionHooks<X>,
    sim: &mut Sim<'_>,
    unit: UnitId,
) {
    let t = h.tables.clone();
    let paths = h.paths.is_some();
    let mut w = UseView {
        cv: CombatView {
            game: sim.game,
            v: View::of(sim.units, sim.stats, sim.data, h),
        },
    };
    let do_it = |w: &mut UseView<'_, X>| {
        if let Some(e) = w.used_skill(unit) {
            let l = skill_level(w, &t.skills, Some(unit), Some(&e), true);
            do_skill(w, &t.skills, unit, e.skill, l);
        }
    };
    let f = if w.used_skill(unit).is_some() {
        w.used_skill_flags(unit)
    } else {
        0
    };
    let mut do_left = true;
    if f & SKILL_MOVING != 0 {
        let stopped = if paths {
            let game = &mut *w.cv.game;
            crate::wiring::path::PathCtx::of(&mut w.cv.v, game).step(unit)
                == Some(crate::path::walk::Step::Stopped)
        } else {
            w.step_path(unit) == 2
        };
        if stopped {
            w.set_used_skill_flags(unit, f | SKILL_ARRIVED);
            do_it(&mut w);
            do_left = false;
        }
    }
    let (code, flags) =
        w.cv.v
            .units
            .get(unit)
            .map_or((0, 0), |r| (r.anim.action_frame, r.flags));
    if code == 4 || (do_left && flags & FLAG_MISSILE_FIRED == 0 && matches!(code, 1 | 2)) {
        do_it(&mut w);
    }
}

/// trigger(U) of the monster attack event (`use.md` §5.2 "Monsters"):
/// U's mode is 14 ? unit flag 0x40 (+0xC4) set : frame code (+0x4E) = 1.
pub fn monster_trigger(units: &crate::units::record::Units, unit: UnitId) -> bool {
    units.get(unit).is_some_and(|r| {
        if r.mode == 14 {
            r.flags & FLAG_MISSILE_FIRED != 0
        } else {
            r.anim.action_frame == 1
        }
    })
}

/// The used-skill branch of the monster attack-family event 0
/// `0x005A7670` (`use.md` §5.2 "Monsters") on the skill use pipeline: E
/// flags bit 0 (a moving skill): the target check and step (`0x00553490`,
/// `0x00554CA0`); stopped → E flags |= 2, the do `0x0056FC50`, and a
/// second do when trigger(U); done. Otherwise (and for a moving skill
/// still on its way) the do, on every event: no frame-code test. The
/// animation refresh that follows is the caller's.
pub fn monster_attack_skill<X: Pending + UseRest>(
    h: &mut ActionHooks<X>,
    sim: &mut Sim<'_>,
    unit: UnitId,
) {
    let t = h.tables.clone();
    let mut w = UseView {
        cv: CombatView {
            game: sim.game,
            v: View::of(sim.units, sim.stats, sim.data, h),
        },
    };
    let do_it = |w: &mut UseView<'_, X>| {
        if let Some(e) = w.used_skill(unit) {
            let l = skill_level(w, &t.skills, Some(unit), Some(&e), true);
            do_skill(w, &t.skills, unit, e.skill, l);
        }
    };
    if w.used_skill(unit).is_none() {
        return;
    }
    let f = w.used_skill_flags(unit);
    if f & SKILL_MOVING != 0 && w.step_path(unit) == 2 {
        w.set_used_skill_flags(unit, f | SKILL_ARRIVED);
        do_it(&mut w);
        // trigger(U) is read after the first do (which may set flag 0x40).
        if monster_trigger(w.cv.v.units, unit) {
            do_it(&mut w);
        }
        return;
    }
    do_it(&mut w);
}

/// The strike of the monster attack-family event 0 `0x005A7670` with no
/// used skill (`use.md` §5.2 "Monsters"): the mode missile `0x005A6D50`
/// (`missiles.md` §R2.2, its argument the moving flag); none → the melee
/// set-up `0x005A5490` and `apply_melee` `0x0057D4F0` (`combat/damage.md`
/// §5.1) on the path target unit `0x00553540`, when there is one.
pub fn monster_attack_strike<X: Pending + UseRest>(
    h: &mut ActionHooks<X>,
    sim: &mut Sim<'_>,
    unit: UnitId,
    moving: bool,
) {
    let t = h.tables.clone();
    let ct = h.tables.combat.clone();
    let mut w = UseView {
        cv: CombatView {
            game: sim.game,
            v: View::of(sim.units, sim.stats, sim.data, h),
        },
    };
    if monster_mode_missile(&mut w, &ct, unit, moving) != 0 {
        return;
    }
    melee_setup(&mut w, &t.skills, &ct, unit);
    if let Some(tg) = crate::wiring::path::monsters::path_target(&*w.cv.v.h, unit) {
        apply_melee(w.combat(), &ct, unit, tg);
    }
}

/// The monster mode damage `0x005A4F50(unit, mode)` of the mode set
/// `0x005A7C20` (`skills/bodies-2.md` §2.1, `umod-callbacks.md` §2 rule
/// 1), on the unit's base list (flag 1).
pub fn monster_mode_damage<X: Pending + UseRest>(
    h: &mut ActionHooks<X>,
    sim: &mut Sim<'_>,
    unit: UnitId,
    mode: u32,
) {
    let t = h.tables.clone();
    let ct = h.tables.combat.clone();
    let mut w = UseView {
        cv: CombatView {
            game: sim.game,
            v: View::of(sim.units, sim.stats, sim.data, h),
        },
    };
    mode_damage(&mut w, &t.skills, &ct, unit, mode as i32);
}

/// The join's Iron Golem re-summon (`formats/d2s-load.md` §3 step 2,
/// `skills/bodies-2b.md` §7.12): the player's skill 90 level L (base +
/// bonuses) and the golem spawned at the player. The saved item has no
/// unit yet, so the body runs without it. True when a golem was made.
/// d2rs-own, unverified (REC-265).
pub fn golem_resummon<X: Pending + UseRest>(
    h: &mut ActionHooks<X>,
    sim: &mut Sim<'_>,
    player: UnitId,
) -> bool {
    const IRON_GOLEM: i32 = 90;
    let t = h.tables.clone();
    let Some(entry) = h
        .skill_lists
        .get(&player)
        .and_then(|l| l.view().into_iter().find(|e| e.skill == IRON_GOLEM))
    else {
        return false;
    };
    let ct = h.tables.combat.clone();
    let mut w = UseView {
        cv: CombatView {
            game: sim.game,
            v: View::of(sim.units, sim.stats, sim.data, h),
        },
    };
    let l = skill_level(&w, &t.skills, Some(player), Some(&entry), true);
    crate::skills::use_::bodies::b3_lvl24::golem_summon(
        &mut w, &t.skills, &ct, player, IRON_GOLEM, l, None,
    ) == 1
}

/// The save load's skill section (`formats/d2s-load.md` §2 "skills":
/// `0x0056A710` → `0x0056DEB0` → assign `0x00647280`,
/// `client/msg-skills.md` §2 rules 1–2, 4): every loaded entry whose
/// skill has a `passivestate` p > 0 gets state p on and its passive
/// stat list refreshed (`0x00646D60`), in list order (the masteries,
/// Increased Stamina, Iron Skin, ... count from the join on).
pub fn passive_refresh_all<X: Pending + UseRest>(
    h: &mut ActionHooks<X>,
    sim: &mut Sim<'_>,
    unit: UnitId,
) {
    let t = h.tables.clone();
    let entries = h
        .skill_lists
        .get(&unit)
        .map(|l| l.view())
        .unwrap_or_default();
    let mut w = UseView {
        cv: CombatView {
            game: sim.game,
            v: View::of(sim.units, sim.stats, sim.data, h),
        },
    };
    for e in entries {
        let p = t
            .skills
            .skill(e.skill)
            .map_or(-1, |r| i32::from(r.passivestate as i16));
        if let Ok(p @ 1..) = u16::try_from(p) {
            w.cv.v.set_state(unit, p, true);
            crate::skills::use_::bodies::BodyWorld::passive_state_apply(&mut w, unit, &e);
        }
    }
}

/// The save load's right-skill selection (`formats/d2s.md` §2.4 rule
/// 6.3, `0x005701B0` hand 0): the selected right skill's aura start
/// ([`crate::skills::use_::right_aura_start`]), so an aura saved on the
/// right button is on from the join (1.14d `pal-vigor` / `pal-fanaticism`
/// checks: the aura's stats at frame 2).
pub fn right_aura_select<X: Pending + UseRest>(
    h: &mut ActionHooks<X>,
    sim: &mut Sim<'_>,
    player: UnitId,
) {
    let t = h.tables.clone();
    let mut w = UseView {
        cv: CombatView {
            game: sim.game,
            v: View::of(sim.units, sim.stats, sim.data, h),
        },
    };
    let Some(e) = UseWorld::right_skill(&w, player) else {
        return;
    };
    let l = skill_level(&w, &t.skills, Some(player), Some(&e), true);
    crate::skills::use_::right_aura_start(&mut w, &t.skills, player, &e, l);
}

/// The Bone Wall maker's `summon_class` (`missiles/bodies-2.md` §33 step
/// 4): `0x0056E620` for `owner` on the skill pipeline.
pub fn missile_summon_class<X: Pending + UseRest>(
    h: &mut ActionHooks<X>,
    sim: &mut Sim<'_>,
    owner: UnitId,
    skill: i32,
    _level: i32,
) -> (i32, i32) {
    let t = h.tables.clone();
    let ct = h.tables.combat.clone();
    let w = UseView {
        cv: CombatView {
            game: sim.game,
            v: View::of(sim.units, sim.stats, sim.data, h),
        },
    };
    crate::skills::use_::bodies::summon_class(&w, &t.skills, &ct, owner, skill)
}

/// The Bone Wall maker's summon spawn (§33 step 7): `summon_spawn`
/// (`skills/bodies.md` §6.2) with flags 0xD, AI special state 0, pet max 0.
#[allow(clippy::too_many_arguments)]
pub fn missile_summon_spawn<X: Pending + UseRest>(
    h: &mut ActionHooks<X>,
    sim: &mut Sim<'_>,
    owner: UnitId,
    class: i32,
    mode: i32,
    at: (i32, i32),
    pet_type: i32,
) -> Option<UnitId> {
    let mut w = UseView {
        cv: CombatView {
            game: sim.game,
            v: View::of(sim.units, sim.stats, sim.data, h),
        },
    };
    crate::skills::use_::bodies::spawn(
        &mut w,
        crate::skills::use_::bodies::Summon {
            flags: 0xD,
            owner,
            class,
            ai: 0,
            mode,
            x: at.0,
            y: at.1,
            pet_type,
            pet_max: 0,
        },
    )
}

/// The Bone Wall maker's piece binding (§33 step 8), in order: owner
/// data (anchor, 0, 0), the anchor's minion list, umod 15, `skill_stats`
/// (ilvl 0), the source-unit link to `owner`, target-node slot 9.
#[allow(clippy::too_many_arguments)]
pub fn missile_bone_wall_piece<X: Pending + UseRest>(
    h: &mut ActionHooks<X>,
    sim: &mut Sim<'_>,
    owner: UnitId,
    anchor: UnitId,
    piece: UnitId,
    skill: i32,
    level: i32,
) {
    use crate::skills::use_::bodies::{self as b, BodyEffect};
    let t = h.tables.clone();
    let mut w = UseView {
        cv: CombatView {
            game: sim.game,
            v: View::of(sim.units, sim.stats, sim.data, h),
        },
    };
    BodyWorld::effect(
        &mut w,
        BodyEffect::OwnerData {
            m: piece,
            owner: Some(anchor),
            a: 0,
            b: 0,
        },
    );
    BodyWorld::effect(
        &mut w,
        BodyEffect::AddMinion {
            leader: anchor,
            m: piece,
        },
    );
    BodyWorld::effect(
        &mut w,
        BodyEffect::Umod {
            m: piece,
            umod: 15,
            arg: 0,
        },
    );
    b::skill_stats(&mut w, &t.skills, owner, piece, skill, level, 0);
    b::link_source(&mut w, piece, Some(owner));
    b::node_prepend(&mut w, piece, 9);
}
