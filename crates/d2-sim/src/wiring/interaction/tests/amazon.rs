// Spec: specs/skills/bodies-2b.md §8.1 (Valkyrie); specs/client/msg-skills.md §2 rule 4 (passive refresh); specs/sim/pets.md §2–§8
//! Amazon skills on the wired host: the Valkyrie summon (`srvdo` 16) and
//! the passive-state refresh `0x00646D60` (Critical Strike, Dodge).

use std::sync::Arc;

use crate::skills::use_::bodies::{BodyStat, BodyTables, BodyWorld};
use crate::skills::use_::do_skill;
use crate::skills::SkillEntry;
use crate::units::{UnitId, UnitType};

use super::skill_bodies::body_rec;
use super::skill_use::Fx;
use super::{stat_data, N_STATES};

/// The `valkyrie` state (93).
const VALKYRIE: u16 = 93;
/// Stat 337 `passive_critical_strike`.
const PASSIVE_CRITICAL_STRIKE: u16 = 337;
/// A passive state of the fixture.
const PASSIVE_STATE: u16 = 50;

/// A formula buffer: `push i16 2; end`, then `push i8 20`.
const F_2: u32 = 0;
const F_20: u32 = 4;

fn tables(r: d2_data::tables::Skills) -> crate::skills::SkillTables {
    let mut t = crate::skills::fake::skill_tables(vec![r]);
    t.skills_code = vec![0x08, 2, 0, 0x00, 0x07, 20, 0x00];
    t
}

fn valkyrie_caster() -> (Fx, UnitId) {
    let mut r = body_rec();
    r.srvdofunc = 16;
    r.summon = 0;
    r.summode = 1;
    r.pettype = 2;
    r.petmax = F_2;
    r.calc2 = F_2;
    let mut fx = Fx::with(stat_data(), tables(r));
    fx.sim.sys.hooks.bodies = Some(Arc::new(BodyTables {
        stats: vec![BodyStat::default(); 359],
        state_group: vec![0; N_STATES],
        state_aura: vec![false; N_STATES],
        pettype_count: 3,
        pettype_group: vec![0; 3],
        ..BodyTables::default()
    }));
    let p = fx.spawn(UnitType::Player, 10, 10);
    fx.set(p, &[(12, 10)]);
    (fx, p)
}

fn entry(skill: i32, base: i32) -> SkillEntry {
    SkillEntry {
        skill,
        base,
        owner_guid: -1,
        ..SkillEntry::default()
    }
}

// Covers: specs/skills/bodies-2b.md §8.1 r3–r5; specs/sim/pets.md §2 r6, §8
#[test]
fn a_valkyrie_is_a_listed_pet_with_state_93() {
    let (mut fx, p) = valkyrie_caster();
    let e = entry(0, 1);
    let x = &mut fx.sim.hooks().x;
    x.skills.insert(p, vec![e]);
    x.used.insert(p, e);
    x.aim_at = (12, 10);
    let t = fx.sim.sys.hooks.tables.skills.clone();
    let game = &mut fx.game;
    assert_eq!(fx.sim.skill_use(game, |w| do_skill(w, &t, p, 0, 1)), 1);
    let m = fx.game.lists.units_of_type(UnitType::Monster);
    assert_eq!(m.len(), 1, "the Valkyrie is a unit");
    let guid = fx.sim.sys.units.get(m[0]).unwrap().guid;
    let listed: Vec<i32> = fx.sim.sys.hooks.pet_lists[&p].entries[2]
        .nodes
        .iter()
        .map(|n| n.guid)
        .collect();
    assert_eq!(listed, vec![guid as i32], "listed under the pet type");
    let on = fx
        .sim
        .with(&mut fx.game, |_, v| v.stats.has_state(m[0], u32::from(VALKYRIE)));
    assert!(on, "state 93 (valkyrie) is on");
    fx.assert_clean();
}

fn passive_player(level: i32) -> (Fx, UnitId, SkillEntry) {
    let mut r = body_rec();
    r.passivestate = PASSIVE_STATE;
    r.passivestat1 = PASSIVE_CRITICAL_STRIKE;
    r.passivecalc1 = F_20;
    r.passivestat2 = 0xFFFF;
    r.passivestat3 = 0xFFFF;
    r.passivestat4 = 0xFFFF;
    r.passivestat5 = 0xFFFF;
    let mut fx = Fx::with(stat_data(), tables(r));
    let p = fx.spawn(UnitType::Player, 10, 10);
    let e = entry(0, level);
    fx.sim.hooks().x.skills.insert(p, vec![e]);
    (fx, p, e)
}

fn stat(fx: &mut Fx, p: UnitId, s: u16) -> i32 {
    fx.sim.with(&mut fx.game, |_, v| v.stat(p, s))
}

// Covers: specs/client/msg-skills.md §2 r4
#[test]
fn a_passive_skill_gives_its_stat_through_the_state_list() {
    let (mut fx, p, e) = passive_player(1);
    assert_eq!(stat(&mut fx, p, PASSIVE_CRITICAL_STRIKE), 0);
    let game = &mut fx.game;
    fx.sim.skill_use(game, |w| {
        w.state_on(p, i32::from(PASSIVE_STATE), true);
        w.passive_state_apply(p, &e);
    });
    assert_eq!(stat(&mut fx, p, PASSIVE_CRITICAL_STRIKE), 20);
    fx.assert_clean();
}

// Covers: specs/client/msg-skills.md §2 r4 (L = 0 frees the list)
#[test]
fn a_passive_skill_at_level_zero_gives_nothing() {
    let (mut fx, p, e) = passive_player(1);
    let game = &mut fx.game;
    fx.sim.skill_use(game, |w| {
        w.state_on(p, i32::from(PASSIVE_STATE), true);
        w.passive_state_apply(p, &e);
    });
    fx.sim.hooks().x.skills.insert(p, vec![entry(0, 0)]);
    let game = &mut fx.game;
    fx.sim.skill_use(game, |w| w.passive_state_apply(p, &e));
    assert_eq!(stat(&mut fx, p, PASSIVE_CRITICAL_STRIKE), 0);
    fx.assert_clean();
}
