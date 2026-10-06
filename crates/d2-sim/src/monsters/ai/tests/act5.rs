// Spec: specs/monsters/ai-bodies-5.md (Test vectors, Edge cases); fakes from the parent test module
use super::act2::{act_row, give_skill, logged, param_of, point_mode, seeded, set_param_of, world};
use super::act3::seed_raw;
use super::npc::{steps_since, unit_mode};
use super::*;

/// Monstats rows up to `n` (copies of row 0) so fixed classes resolve.
fn rows(w: &mut World, n: usize) {
    let r = w.monstats[0].clone();
    w.monstats.resize(n, r);
    let m = w.modes[0];
    w.modes.resize(n, m);
}

// ---- §2 Minion ---------------------------------------------------------

// Covers: specs/monsters/ai-bodies-5.md §2 text, §2 r1, §2 r2, §2 r3, §edge-cases-original-bugs r2
#[test]
fn minion_vectors() {
    let m1 = [70, 15, 50, 15];
    // No command, C: 50 (< 70), 10 (< 15) → A2.
    let (mut w, lo) = seeded(act_row(116, &m1), 2, |v| v[0] < 70 && v[1] < 15);
    w.think_with(Some(w.player), 1, true);
    assert_eq!(w.fake.modes(), [unit_mode(mode::ATTACK2, w.player)]);
    assert_eq!(steps_since(&w, lo), 2);
    // An order (type 1, frame < param 3) on another living unit: F := 1,
    // no stall draw, walk to it (not in melee).
    let mut w = world(act_row(116, &m1));
    w.seed(1);
    let v = w.add_unit(UnitType::Monster, (120, 100));
    let g = w.game.lists.unit(v).unwrap().guid as i32;
    w.store.control_mut(w.mon).unwrap().commands = vec![AiCommand {
        params: [1, 1, g, 50, 0],
    }];
    w.think_with(Some(w.player), 1, true);
    assert_eq!(w.fake.modes(), [unit_mode(mode::WALK, v)]);
    assert_eq!(steps_since(&w, 1), 0);
    assert_eq!(w.store.control(w.mon).unwrap().commands.len(), 1);
    // An expired order is freed.
    let mut w = world(act_row(116, &m1));
    w.game.frame = 60;
    w.store.control_mut(w.mon).unwrap().commands = vec![AiCommand {
        params: [1, 1, g, 50, 0],
    }];
    w.think_with(Some(w.player), 1, true);
    assert!(w.store.control(w.mon).unwrap().commands.is_empty());
}

// ---- §3 Imp ------------------------------------------------------------

// Covers: specs/monsters/ai-bodies-5.md §3 r0, §3 l2 r1, §3 l2 r3, §3 l2 r6, §edge-cases-original-bugs r1
#[test]
fn imp_rides_and_teleports() {
    let mut w = world(act_row(122, &[]));
    rows(&mut w, 496);
    // The aips come from rows 492..495 whatever the unit's row.
    w.monstats[492].aip1 = 25;
    w.monstats[492].aip2 = 14;
    w.monstats[492].aip3 = 10;
    w.monstats[493].aip1 = 5;
    give_skill(&mut w, 1, 264, 10);
    let mon = w.mon;
    // The init: m := −1.
    w.with(|g, cx| run_init(g, cx, 0x005E_2FD0, mon));
    assert_eq!(param_of(&w, 0), -1);
    // A mount far away (squared 36 > 25): walk to it.
    let b = w.add_unit(UnitType::Monster, (106, 100));
    let g = w.game.lists.unit(b).unwrap().guid as i32;
    set_param_of(&mut w, 0, g);
    w.think_with(Some(w.player), 10, false);
    assert_eq!(w.fake.modes(), [unit_mode(mode::WALK, b)]);
    // Near (squared 16): `Skill1` at it.
    w.fake.pos.insert(b, (104, 100));
    w.think_with(Some(w.player), 10, false);
    assert_eq!(w.fake.modes().last().unwrap(), &unit_mode(10, b));
    // The mount has an owner: m := −1.
    w.fake.x.owners.insert(b, w.player);
    w.seed(1);
    w.think_with(Some(w.player), 10, false);
    assert_eq!(param_of(&w, 0), -1);
    // Draw < I1.aip3: teleport in range 14: two draws over 28.
    let lo = seed_raw(3, |v| v[0] % 100 < 10);
    let mut w = world(act_row(122, &[]));
    rows(&mut w, 496);
    w.monstats[492].aip2 = 14;
    w.monstats[492].aip3 = 10;
    give_skill(&mut w, 1, 264, 10);
    set_param_of(&mut w, 0, -1);
    w.seed(lo);
    w.think_with(Some(w.player), 10, false);
    let mut s = Seed::init_low(lo);
    s.step();
    let x = 100 + (s.step() % 28) as i32 - 14;
    let y = 100 + (s.step() % 28) as i32 - 14;
    assert_eq!(w.fake.modes(), [point_mode(10, x, y)]);
}

// ---- §4 Succubus, §6 SuccubusWitch -------------------------------------

// Covers: specs/monsters/ai-bodies-5.md §4 text, §4 r1, §4 r2, §4 r3, §4 r4
#[test]
fn succubus_curses() {
    let s1 = [90, 50, 50, 15, 15, 15, 3, 0];
    // T not cursed, D < aip4, P(aip3): `Skill1` (> 0) at T (life ≥ aip7).
    let (mut w, _) = seeded(act_row(118, &s1), 1, |v| v[0] < 50);
    give_skill(&mut w, 1, 87, 10);
    w.think_with(Some(w.player), 5, false);
    assert_eq!(w.fake.modes(), [unit_mode(10, w.player)]);
    // A cursed T: no curse; C → P(aip1) → A1.
    let (mut w, _) = seeded(act_row(118, &s1), 1, |v| v[0] < 90);
    give_skill(&mut w, 1, 87, 10);
    w.fake.x.cursed.insert(w.player);
    w.think_with(Some(w.player), 1, true);
    assert_eq!(w.fake.modes(), [unit_mode(mode::ATTACK1, w.player)]);
    // `Skill3` when T's max mana < max life.
    let (mut w, _) = seeded(act_row(118, &s1), 1, |v| v[0] < 50);
    give_skill(&mut w, 3, 89, 10);
    w.fake.x.max_life.insert(w.player, 100);
    w.think_with(Some(w.player), 5, false);
    assert_eq!(w.fake.modes(), [unit_mode(10, w.player)]);
    // Skill id 0 is "no skill" here (tests are > 0): walk by P(aip2).
    let (mut w, _) = seeded(act_row(118, &s1), 2, |v| v[0] < 50 && v[1] < 50);
    give_skill(&mut w, 1, 0, 10);
    w.think_with(Some(w.player), 5, false);
    assert_eq!(w.fake.modes(), [unit_mode(mode::WALK, w.player)]);
}

// Covers: specs/monsters/ai-bodies-5.md §6 text, §6 r1, §6 r2, §6 r3
#[test]
fn succubus_witch_rules() {
    let sw = [90, 25, 30, 6, 90, 15, 80, 66];
    // C, draw < aip3, the escape by aip4 starts → end.
    let (mut w, _) = seeded(act_row(119, &sw), 1, |v| v[0] < 30);
    w.fake.x.cursed.insert(w.player);
    w.think_with(Some(w.player), 1, true);
    assert_eq!(w.fake.modes(), [point_mode(mode::WALK, 94, 100)]);
    // Not C, `Skill5`, aip8 > 0: draw < aip5, S, draw < aip8 → `Skill5` at S.
    let (mut w, _) = seeded(act_row(119, &sw), 2, |v| v[0] < 90 && v[1] < 66);
    give_skill(&mut w, 5, 74, 10);
    w.fake.x.cursed.insert(w.player);
    let s = w.add_unit(UnitType::Player, (110, 100));
    w.fake.secondary = Some((s, 10));
    w.think_with(Some(w.player), 10, false);
    assert_eq!(w.fake.modes(), [unit_mode(10, s)]);
    // No `Skill5`: S2 at S by draw < aip5.
    let (mut w, _) = seeded(act_row(119, &sw), 1, |v| v[0] < 90);
    w.fake.x.cursed.insert(w.player);
    let s = w.add_unit(UnitType::Player, (110, 100));
    w.fake.secondary = Some((s, 10));
    w.think_with(Some(w.player), 10, false);
    assert_eq!(w.fake.modes(), [unit_mode(mode::SKILL2, s)]);
}

// ---- §5 BloodLord, §10 DeathMauler, §14 FrozenHorror ------------------

// Covers: specs/monsters/ai-bodies-5.md §5 text, §5 r1, §5 r2
#[test]
fn blood_lord_vectors() {
    let bl = [90, 85, 50, 10];
    // C: 50 (< 90), 60 (≥ 50) → A1.
    let (mut w, _) = seeded(act_row(125, &bl), 2, |v| v[0] < 90 && v[1] >= 50);
    w.think_with(Some(w.player), 1, true);
    assert_eq!(w.fake.modes(), [unit_mode(mode::ATTACK1, w.player)]);
    // Draw < aip3: the frenzy in mode 5, `Skill1` untested.
    let (mut w, _) = seeded(act_row(125, &bl), 2, |v| v[0] < 90 && v[1] < 50);
    w.think_with(Some(w.player), 1, true);
    assert!(logged(&w, "skill -1"));
    assert_eq!(w.fake.modes(), [unit_mode(mode::ATTACK2, w.player)]);
}

// Covers: specs/monsters/ai-bodies-5.md §10 text, §10 r1, §10 r2, §10 r3
#[test]
fn death_mauler_vectors() {
    let dm = [75, 65, 15, 50];
    // C = 0, D = 10: 30 (< 50) → DeathMaul at T.
    let (mut w, lo) = seeded(act_row(130, &dm), 1, |v| v[0] < 50);
    give_skill(&mut w, 1, 365, 8);
    w.think_with(Some(w.player), 10, false);
    assert_eq!(w.fake.modes(), [unit_mode(8, w.player)]);
    assert_eq!(steps_since(&w, lo), 1);
    // C: P(aip1) fails → idle 15.
    let (mut w, _) = seeded(act_row(130, &dm), 1, |v| v[0] >= 75);
    w.think_with(Some(w.player), 1, true);
    assert_eq!(w.thinks(), [15]);
}

// Covers: specs/monsters/ai-bodies-5.md §14 text, §14 r1, §14 r2, §14 r3, §14 r4
#[test]
fn frozen_horror_vectors() {
    let fh = [60, 40, 60, 10];
    // `Skill1` level 4, D = 3, no state 12: 50 (< 60) → Arctic Blast at T.
    let (mut w, _) = seeded(act_row(124, &fh), 1, |v| v[0] < 60);
    give_skill(&mut w, 1, 372, 10);
    w.fake.x.skill_level.insert(372, 4);
    w.think_with(Some(w.player), 3, false);
    assert_eq!(w.fake.modes(), [unit_mode(10, w.player)]);
    // D ≥ R: not C, P(aip2) fails → idle aip4; state 12 cleared.
    let (mut w, _) = seeded(act_row(124, &fh), 1, |v| v[0] >= 40);
    give_skill(&mut w, 1, 372, 10);
    w.fake.x.skill_level.insert(372, 4);
    w.fake.states.insert((w.mon, 12));
    w.think_with(Some(w.player), 4, false);
    assert!(!w.fake.states.contains(&(w.mon, 12)));
    assert_eq!(w.thinks(), [10]);
}

// ---- §7 Overseer -------------------------------------------------------

// Covers: specs/monsters/ai-bodies-5.md §7 text, §7 r1, §7 r2, §7 r3, §7 r5, §7 r6, §7 r8, §7 r9
#[test]
fn overseer_rules() {
    let ov = [250, 50, 50, 17, 7, 100, 50];
    // A hurt minion1 within 24: Healing Vortex by draw < aip2 and "has 2".
    let (mut w, _) = seeded(act_row(120, &ov), 1, |v| v[0] < 50);
    give_skill(&mut w, 2, 380, 10);
    w.monstats[0].baseid = 453;
    let m = w.add_unit(UnitType::Monster, (110, 100));
    w.fake.life_of.insert(m, 30);
    w.think_with(Some(w.player), 30, false);
    assert!(logged(&w, "quest Shenk"));
    assert_eq!(w.fake.modes(), [unit_mode(10, m)]);
    // Cry Help at the hostile current target in AI state 3; cooldown.
    let mut w = world(act_row(120, &ov));
    give_skill(&mut w, 1, 379, 10);
    w.fake.ai_state = 3;
    w.fake.x.target_unit = Some(w.player);
    w.game.frame = 5;
    w.think_with(Some(w.player), 30, false);
    assert_eq!(w.fake.modes(), [unit_mode(10, w.player)]);
    assert_eq!(param_of(&w, 0), 255);
    // No minions: draw ≥ 60 → idle 10.
    let (mut w, _) = seeded(act_row(120, &ov), 1, |v| v[0] >= 60);
    w.think_with(Some(w.player), 30, false);
    assert_eq!(w.thinks(), [10]);
    // With a minion, D < aip4 − aip5: escape by aip4 − D.
    let mut w = world(act_row(120, &ov));
    w.monstats[0].baseid = 453;
    w.add_unit(UnitType::Monster, (90, 90));
    w.fake.unique = true;
    w.think_with(Some(w.player), 5, false);
    assert_eq!(w.fake.modes(), [point_mode(mode::WALK, 88, 100)]);
}

// ---- §8 ReanimatedHorde, §9 ClawViperEx ----------------------------------

// Covers: specs/monsters/ai-bodies-5.md §8 text, §8 r1, §8 r2, §8 r3, §8 r4
#[test]
fn reanimated_horde_rules() {
    let rh = [30, 20, 12, 20, 20, 65, 25];
    // Every think sets unit flags 0x0E; charge when 5 < D < aip3.
    let (mut w, _) = seeded(act_row(114, &rh), 1, |v| v[0] < 20);
    give_skill(&mut w, 2, 107, 14);
    w.think_with(Some(w.player), 8, false);
    assert!(logged(&w, "flag 0xe"));
    assert_eq!(w.fake.modes(), [unit_mode(14, w.player)]);
    // D = 5: no charge draw; 20 ≤ draw < 65 → walk in radius (4, 0).
    let (mut w, lo) = seeded(act_row(114, &rh), 2, |v| v[0] >= 20 && v[1] < 65);
    give_skill(&mut w, 2, 107, 14);
    w.think_with(Some(w.player), 5, false);
    assert!(logged(&w, "radius 4 0"));
    assert_eq!(steps_since(&w, lo), 2);
}

// Covers: specs/monsters/ai-bodies-5.md §9 text, §9 r1, §9 r2, §9 r3, §9 r4, §9 r5
#[test]
fn claw_viper_ex_rules() {
    let cv6 = [60, 10, 90, 85, 5, 1, 18, 50];
    // D < aip7, P(aip4), frame > param 1 → A1, cooldown aip8.
    let (mut w, _) = seeded(act_row(142, &cv6), 1, |v| v[0] < 85);
    w.game.frame = 10;
    w.think_with(Some(w.player), 12, false);
    assert_eq!(w.fake.modes(), [unit_mode(mode::ATTACK1, w.player)]);
    assert_eq!(param_of(&w, 1), 60);
    // In cooldown: idle aip5.
    let (mut w, _) = seeded(act_row(142, &cv6), 1, |v| v[0] < 85);
    w.game.frame = 10;
    set_param_of(&mut w, 1, 60);
    w.think_with(Some(w.player), 12, false);
    assert_eq!(w.thinks(), [15]);
    // P(aip4) fails, D < aip2, P(aip1), the check: the charge (state 90
    // on, param 0 := 1).
    let (mut w, _) = seeded(act_row(142, &cv6), 2, |v| v[0] >= 85 && v[1] < 60);
    give_skill(&mut w, 1, 107, 14);
    w.fake.pos.insert(w.player, (120, 100));
    w.think_with(Some(w.player), 5, false);
    assert!(logged(
        &w,
        &format!("check 107 Some({:?}) 120 100", w.player)
    ));
    assert!(w.fake.states.contains(&(w.mon, 90)));
    assert_eq!(param_of(&w, 0), 1);
}

// ---- §11 PutridDefiler -------------------------------------------------

// Covers: specs/monsters/ai-bodies-5.md §11 text, §11 r1, §11 r2, §11 r3, §11 r4
#[test]
fn putrid_defiler_impregnates() {
    // Row 0 the defiler (`BaseId` 546, so not its own host), row 1 a host
    // class.
    let setup = |at: (i32, i32)| {
        let mut w = world(act_row(137, &[15, 5]));
        w.monstats[0].baseid = 546;
        w.monstats.push(act_row(3, &[]));
        let h = w.add_unit(UnitType::Monster, at);
        w.fake.class.insert(h, 1);
        (w, h)
    };
    // A host in melee range: skill 300 in mode 8 at it.
    let (mut w, h) = setup((102, 100));
    w.fake.melee.insert(h);
    w.think_with(Some(w.player), 10, false);
    assert!(logged(&w, "skill 300"));
    assert_eq!(w.fake.modes(), [unit_mode(mode::SKILL1, h)]);
    // A pregnant host does not count; no host, D < aip1 → escape by 5.
    let (mut w, h) = setup((102, 100));
    w.fake.states.insert((h, 110));
    w.think_with(Some(w.player), 10, false);
    assert_eq!(w.fake.modes(), [point_mode(mode::WALK, 95, 100)]);
    // Not in melee: walk to the host.
    let (mut w, h) = setup((110, 100));
    w.think_with(Some(w.player), 20, false);
    assert_eq!(w.fake.modes(), [unit_mode(mode::WALK, h)]);
    // Beyond full-size distance 25: no host; D ≥ aip1 → idle 25.
    let (mut w, _) = setup((140, 100));
    w.think_with(Some(w.player), 20, false);
    assert_eq!(w.thinks(), [25]);
}

// ---- §12 Ancient, §13 AncientStatue --------------------------------------

// Covers: specs/monsters/ai-bodies-5.md §12 text, §12 r1, §12 r2, §12 r3, §12 r4, §12 r5, §12 l3 r1
#[test]
fn ancients_rules() {
    let talic = [15, 25, 75, 8];
    let setup = |class: i32| {
        let mut w = world(act_row(133, &talic));
        rows(&mut w, 543);
        w.fake.class.insert(w.mon, class);
        w.fake.nodes = vec![vec![w.player]];
        w
    };
    // Other classes: nothing scheduled.
    let mut w = world(act_row(133, &talic));
    w.think_with(Some(w.player), 5, false);
    assert!(w.thinks().is_empty() && w.fake.modes().is_empty());
    // The gate: idle 25.
    let mut w = setup(540);
    w.fake.x.quests.insert("AncientsNotActivatable".into());
    w.think_with(Some(w.player), 5, false);
    assert_eq!(w.thinks(), [25]);
    // Talic: T out of melee → walk to it, wait 10.
    let mut w = setup(540);
    w.think_with(Some(w.player), 5, false);
    assert_eq!(w.fake.modes(), [unit_mode(mode::WALK, w.player)]);
    assert_eq!(w.thinks(), [10]);
    // The whirlwind: reach aip4 = 8 along the line to X.
    let (mut w, _) = seeded(act_row(133, &talic), 1, |v| v[0] < 25);
    rows(&mut w, 543);
    w.fake.class.insert(w.mon, 540);
    for r in w.monstats.iter_mut() {
        r.skill1 = 151;
    }
    for m in w.modes.iter_mut() {
        m[0] = 14;
    }
    w.think_with(Some(w.player), 5, false);
    assert_eq!(w.fake.modes(), [point_mode(14, 113, 100)]);
    // Korlic with no T: the pick (the node), in melee → A1 / A2 by roll(2).
    let lo = seed_raw(2, |v| v[0] % 100 < 75);
    let mut w = setup(542);
    w.seed(lo);
    w.fake.melee.insert(w.player);
    w.think_with(None, 0, false);
    let mut s = Seed::init_low(lo);
    s.step();
    let m = if !s.step().is_multiple_of(2) {
        mode::ATTACK1
    } else {
        mode::ATTACK2
    };
    assert_eq!(w.fake.modes(), [unit_mode(m, w.player)]);
}

// Covers: specs/monsters/ai-bodies-5.md §13
#[test]
fn ancient_statue_spawns() {
    let mut w = world(act_row(132, &[]));
    w.think_with(None, 0, false);
    assert_eq!(w.thinks(), [25]);
    let mut w = world(act_row(132, &[]));
    w.fake.x.quests.insert("AncientsActivatable".into());
    w.think_with(None, 0, false);
    assert!(logged(&w, "portalflag"));
    assert!(logged(&w, "skill 302"));
    assert_eq!(w.fake.modes(), [point_mode(mode::ATTACK1, 100, 100)]);
}

// ---- §15 SiegeBeast ----------------------------------------------------

// Covers: specs/monsters/ai-bodies-5.md §15 text, §15 r1, §15 r2, §15 r3, §15 r5, §15 r6, §edge-cases-original-bugs r3
#[test]
fn siege_beast_rules() {
    let sb = [25, 50, 1, 15, 1, 50, 100];
    // Dying: nothing.
    let mut w = world(act_row(115, &sb));
    w.fake.anim.insert(w.mon, mode::DEATH);
    w.think_with(Some(w.player), 5, false);
    assert!(w.thinks().is_empty());
    // A free imp (`BaseId` 492, param 0 = −1) within 25: its param 0 := the
    // beast's GUID.
    let mut w = world(act_row(115, &sb));
    rows(&mut w, 493);
    let imp = w.add_unit(UnitType::Monster, (110, 100));
    w.fake.class.insert(imp, 492);
    w.monstats[492].baseid = 492;
    w.store.entry(imp).control = Some(AiControl {
        params: [-1, 0, 0],
        ..AiControl::default()
    });
    w.think_with(Some(w.player), 20, false);
    let g = w.game.lists.unit(w.mon).unwrap().guid as i32;
    assert_eq!(w.store.control(imp).unwrap().params[0], g);
    // Direct line, draw < aip6: velocity (0, aip7, 0), walk in radius.
    assert!(logged(&w, "radius 12 0"));
    // C: draw ≥ aip3 (no stomp), < aip2 → A1.
    let (mut w, _) = seeded(act_row(115, &sb), 2, |v| v[0] >= 1 && v[1] < 50);
    give_skill(&mut w, 1, 384, 10);
    w.think_with(Some(w.player), 1, true);
    assert_eq!(w.fake.modes(), [unit_mode(mode::ATTACK1, w.player)]);
}

// ---- §16 SuicideMinion, §17 BaalMinion -----------------------------------

// Covers: specs/monsters/ai-bodies-5.md §16 text, §16 r1, §16 r2, §16 r3, §edge-cases-original-bugs r6
#[test]
fn suicide_minion_vectors() {
    let sm = [15, 5, 50, 4, 0];
    // f = 0, C: f := frame, idle 5; next think: death.
    let mut w = world(act_row(117, &sm));
    w.seed(1);
    w.game.frame = 100;
    w.think_with(Some(w.player), 1, true);
    assert_eq!((param_of(&w, 0), w.thinks()), (100, vec![105]));
    w.game.frame = 105;
    w.think_with(Some(w.player), 1, true);
    assert_eq!(w.fake.modes(), [unit_mode(mode::DEATH, w.player)]);
    assert_eq!(steps_since(&w, 1), 0);
    // f ≠ 0, not yet: idle aip2.
    let mut w = world(act_row(117, &sm));
    w.game.frame = 100;
    set_param_of(&mut w, 0, 100);
    w.think_with(Some(w.player), 1, true);
    assert_eq!(w.thinks(), [105]);
}

// Covers: specs/monsters/ai-bodies-5.md §17 text, §17 r1, §17 r2, §17 r3
#[test]
fn baal_minion_always_waits() {
    let bm = [90, 85, 50, 17];
    // Not C: draw < aip2 → walk; then wait aip4.
    let (mut w, _) = seeded(act_row(141, &bm), 1, |v| v[0] < 85);
    w.think_with(Some(w.player), 9, false);
    assert_eq!(w.fake.modes(), [unit_mode(mode::WALK, w.player)]);
    assert_eq!(w.thinks(), [17]);
    // C, draw ≥ aip1: idle aip3, replaced by the wait aip4.
    let (mut w, _) = seeded(act_row(141, &bm), 1, |v| v[0] >= 90);
    w.think_with(Some(w.player), 1, true);
    assert_eq!(w.thinks(), [17]);
}

// ---- §18 BaalTaunt, §19 BaalToStairs -------------------------------------

// Covers: specs/monsters/ai-bodies-5.md §18 text, §18 r1, §18 r2, §18 r3, §18 r4, §18 r5
#[test]
fn baal_taunt_vectors() {
    let bt = [3, 10, 20];
    // T a player in mode 1, n = 10: n := 0, taunt (skill 284, mode 4) at T.
    let mut w = world(act_row(136, &bt));
    set_param_of(&mut w, 0, 10);
    w.think_with(Some(w.player), 5, false);
    assert_eq!(param_of(&w, 0), 0);
    assert!(logged(&w, "skill 284"));
    assert_eq!(w.fake.modes(), [unit_mode(mode::ATTACK1, w.player)]);
    // T in another mode: n := 0; D > aip3 and the placement works → idle 25.
    let mut w = world(act_row(136, &bt));
    set_param_of(&mut w, 0, 4);
    w.fake.anim.insert(w.player, mode::WALK);
    w.fake.x.place_ok = true;
    w.think_with(Some(w.player), 30, false);
    assert_eq!(param_of(&w, 0), 0);
    assert!(logged(&w, "place 105 100"));
    assert_eq!(w.thinks(), [25]);
    // No T: idle 25.
    let mut w = world(act_row(136, &bt));
    w.think_with(None, 0, false);
    assert_eq!(w.thinks(), [25]);
}

// Covers: specs/monsters/ai-bodies-5.md §19 text, §19 r1, §19 r2, §19 r3
#[test]
fn baal_to_stairs_vanishes() {
    let mut w = world(act_row(138, &[4]));
    // No portal object: idle 25.
    w.think_with(None, 0, false);
    assert_eq!(w.thinks(), [25]);
    // The portal (object class 563) at 3: the quest call, invis, removal.
    let o = w.add_unit(UnitType::Object, (103, 100));
    w.fake.class.insert(o, 563);
    w.think_with(None, 0, false);
    assert!(logged(&w, "quest BaalToStairs"));
    assert!(w.fake.states.contains(&(w.mon, 146)));
    assert!(logged(&w, &format!("remove {}", w.mon.0)));
    // At 10: velocity (7, 0, 0), walk to it.
    let mut w = world(act_row(138, &[4]));
    let o = w.add_unit(UnitType::Object, (110, 100));
    w.fake.class.insert(o, 563);
    w.think_with(None, 0, false);
    assert_eq!(w.vel_request().method, 7);
    assert_eq!(w.fake.modes(), [unit_mode(mode::WALK, o)]);
}

// ---- §20 BaalThrone ----------------------------------------------------

// Covers: specs/monsters/ai-bodies-5.md §20 text, §20 r2, §20 r3, §20 r5, §20 r6, §20 r7, §edge-cases-original-bugs r4, §edge-cases-original-bugs r5
#[test]
fn baal_throne_waves() {
    let mut w = world(act_row(134, &[25]));
    let mon = w.mon;
    w.game.frame = 100;
    // n = 0, flag 1 clear, wave 0 (class 62): preload 62 and 23, corpse
    // explosion, flags |= 1, q := frame + 250, sound 16.
    w.fake.x.waves.insert(0, (61, 62));
    w.think_with(None, 0, false);
    assert!(logged(&w, "preload 62") && logged(&w, "preload 23"));
    assert!(logged(&w, "skill 285"));
    assert!(logged(&w, "sound 16"));
    assert_eq!(w.store.control(mon).unwrap().params, [0, 1, 350]);
    // frame < q: idle by frame − q (negative): the timer queue puts the
    // past frame 100 + (100 − 350) at frame + 1 (`tick.md` §5).
    w.think_with(None, 0, false);
    assert_eq!(w.thinks(), [101]);
    // At q: spawn wave 0 at (x, y + 13), w := 1, q := frame + 100.
    w.game.frame = 350;
    w.think_with(None, 0, false);
    assert!(logged(&w, "assign 286 1"));
    assert!(logged(&w, "skillparam 286 61"));
    assert_eq!(w.fake.modes().last().unwrap(), &point_mode(10, 100, 113));
    assert_eq!(w.store.control(mon).unwrap().params, [1, 2, 450]);
    // A living ally: no wave; idle 10.
    let a = w.add_unit(UnitType::Monster, (120, 100));
    let _ = a;
    w.game.frame = 500;
    w.think_with(None, 0, false);
    assert_eq!(w.thinks(), [510]);
    // No wave record: nothing scheduled.
    let mut w = world(act_row(134, &[25]));
    w.think_with(None, 0, false);
    assert!(w.thinks().is_empty() && w.fake.modes().is_empty());
    // w ≥ 5: reinit as 559 in mode 1, special state 0, state 142.
    let mut w = world(act_row(134, &[25]));
    w.store.control_mut(w.mon).unwrap().params = [5, 1, 0];
    w.think_with(None, 0, false);
    assert!(logged(&w, "reinit 559 1"));
    assert!(logged(&w, "classlist 559"));
    assert!(w.fake.states.contains(&(w.mon, 142)));
    assert_eq!(w.thinks(), [5]);
}

// ---- §21 BaalCrab, §22 BaalCrabClone ------------------------------------

// Covers: specs/monsters/ai-bodies-5.md §21 text, §21 r1, §21 r2, §21 r3, §21 r4, §21.2 r1, §21.2 r2, §21.3
#[test]
fn baal_crab_executes_then_waits() {
    // X = 0, no collision, draw < 8 → 9: skill 315 in mode 9; then wait 25.
    let (mut w, _) = seeded(act_row(135, &[]), 1, |v| v[0] < 8);
    w.think_with(None, 0, false);
    assert!(logged(&w, "skill 315"));
    assert_eq!(w.fake.modes(), [point_mode(mode::SKILL2, 0, 0)]);
    assert_eq!(w.thinks(), [25]);
    // Draw ≥ 8 → 1: idle 35 on Normal, replaced by the wait 25.
    let (mut w, _) = seeded(act_row(135, &[]), 1, |v| v[0] >= 8);
    w.think_with(None, 0, false);
    assert_eq!(w.thinks(), [25]);
    // A pending choice 13: skill 318 in mode 4 at X.
    let mut w = world(act_row(135, &[]));
    w.fake.nodes = vec![vec![w.player]];
    w.monstats[0].threat = 5;
    set_param_of(&mut w, 0, 13);
    w.think_with(None, 0, false);
    assert!(logged(&w, "skill 318"));
    assert_eq!(w.fake.modes(), [unit_mode(mode::ATTACK1, w.player)]);
    // Choice 14 (teleport 25 away): the free spot for the class at P.
    let mut w = world(act_row(135, &[]));
    w.fake.nodes = vec![vec![w.player]];
    w.monstats[0].threat = 5;
    give_skill(&mut w, 5, 277, 10);
    set_param_of(&mut w, 0, 14);
    w.fake.x.free_spot = Some((70, 101));
    w.think_with(None, 0, false);
    assert!(logged(&w, "freespot 0 75 100"));
    assert_eq!(w.fake.modes(), [point_mode(10, 70, 101)]);
}

// Covers: specs/monsters/ai-bodies-5.md §21.2 r3, §21.2 r4, §21.2 r5
#[test]
fn baal_choice_weights_in_melee() {
    // X in melee, clear, Normal, L = 100: W = {0, 75, 0, 0, 0, 0, 0, 0, 20,
    // 30, 150, 10, 10, 45, 40, 0}, Σ = 380. r = 76 → cumulative 75, 95 →
    // k = 8: `Skill6` (> 0) at X (a player whose max mana < max life);
    // r = 74 → k = 1: idle 35 (no mode request). Then the wait 25.
    for (r, cast) in [(76, true), (74, false)] {
        let lo = seed_raw(1, |v| v[0] % 380 == r);
        let mut w = world(act_row(135, &[]));
        w.seed(lo);
        w.fake.nodes = vec![vec![w.player]];
        w.monstats[0].threat = 5;
        w.fake.melee.insert(w.player);
        w.fake.x.max_life.insert(w.player, 100);
        give_skill(&mut w, 6, 87, 10);
        w.think_with(None, 0, false);
        if cast {
            assert_eq!(w.fake.modes(), [unit_mode(10, w.player)], "r {r}");
        } else {
            assert!(w.fake.modes().is_empty(), "r {r}");
        }
        assert_eq!(w.thinks(), [25]);
    }
    // X's life < 33 %: W10 := 200 (Σ = 430); r = 300 → cumulative 95, 125,
    // 325 → k = 10: A2 at X.
    let lo = seed_raw(1, |v| v[0] % 430 == 300);
    let mut w = world(act_row(135, &[]));
    w.seed(lo);
    w.fake.nodes = vec![vec![w.player]];
    w.monstats[0].threat = 5;
    w.fake.melee.insert(w.player);
    w.fake.life_of.insert(w.player, 20);
    w.fake.life_of.insert(w.mon, 100);
    w.think_with(None, 0, false);
    assert_eq!(w.fake.modes(), [unit_mode(mode::ATTACK2, w.player)]);
}

// Covers: specs/monsters/ai-bodies-5.md §21.3
#[test]
fn baal_clone_spawns_a_third() {
    let mut w = world(act_row(135, &[]));
    rows(&mut w, 571);
    let room = w.room;
    let mon = w.mon;
    let c = w.add_unit(UnitType::Monster, (90, 90));
    w.store.entry(c).control = Some(AiControl::default());
    w.fake.x.room_at = Some(room);
    w.fake.x.spawn = Some(c);
    w.fake.x.max_life.insert(mon, 3000);
    w.fake.stats.insert((mon, 6), 1500);
    set_param_of(&mut w, 0, 15);
    let lo = 77;
    w.seed(lo);
    w.think_with(None, 0, false);
    let mut s = Seed::init_low(lo);
    let x = 100 + (s.step() % 24) as i32 - 12;
    let y = 100 + (s.step() % 24) as i32 - 12;
    assert!(logged(&w, &format!("spawn 570 {x} {y} 1 -1 0x0")));
    assert!(logged(&w, "flag 0x4020000"));
    assert_eq!(w.fake.stats[&(c, 7)], 1000);
    assert_eq!(w.fake.stats[&(c, 6)], 500);
    assert_eq!(w.fake.stats[&(c, 74)], 0);
    assert_eq!(param_of(&w, 1), 1);
    let cg = w.game.lists.unit(c).unwrap().guid;
    assert_eq!(w.store.control(mon).unwrap().minions, [cg]);
    // A living minion: no second clone.
    let n = w.fake.log.len();
    set_param_of(&mut w, 0, 15);
    w.think_with(None, 0, false);
    assert!(!w.fake.log[n..].iter().any(|l| l.starts_with("spawn")));
}

// Covers: specs/monsters/ai-bodies-5.md §22 text, §22 r1, §22 r2, §22 r3, §22 r4
#[test]
fn baal_crab_clone_dies_with_its_owner() {
    // No owner: killed.
    let mut w = world(act_row(140, &[]));
    w.think_with(None, 0, false);
    assert!(logged(&w, &format!("kill {} None", w.mon.0)));
    // A living owner, no X: idle 15.
    let mut w = world(act_row(140, &[]));
    w.fake.x.owners.insert(w.mon, w.player);
    w.think_with(None, 0, false);
    assert_eq!(w.thinks(), [15]);
    // Choices 7, 9, 14, 15 become 2 (walk in radius of X).
    let mut w = world(act_row(140, &[]));
    w.fake.x.owners.insert(w.mon, w.player);
    w.fake.nodes = vec![vec![w.player]];
    w.monstats[0].threat = 5;
    set_param_of(&mut w, 0, 14);
    w.think_with(None, 0, false);
    assert!(logged(&w, "radius 12 0"));
    assert_eq!(w.thinks(), [25]);
}

// ---- §23 Nihlathak -----------------------------------------------------

// Covers: specs/monsters/ai-bodies-5.md §23 text, §23 r1, §23 r2, §23 r3, §23 r4, §23 r5, §23 r8, §23 r10
#[test]
fn nihlathak_rules() {
    let nih = [30, 20, 80, 75, 8];
    // No T, no entry: idle 25; the quest call and state 12 off first.
    let mut w = world(act_row(128, &nih));
    w.fake.states.insert((w.mon, 12));
    w.think_with(None, 0, false);
    assert!(logged(&w, "quest Nihlathak"));
    assert!(!w.fake.states.contains(&(w.mon, 12)));
    assert_eq!(w.thinks(), [25]);
    // C, E1, draw < aip1: blink in range aip2 with the entry's id and mode.
    let (mut w, _) = seeded(act_row(128, &nih), 1, |v| v[0] < 30);
    give_skill(&mut w, 1, 264, 10);
    w.fake.x.skill_entry.insert(264, (264, 11));
    w.think_with(Some(w.player), 1, true);
    assert!(logged(&w, "skill 264"));
    assert!(w.fake.modes()[0].starts_with("mode 11 Point"));
    // `Skill2` > 0, the whip draw fails: the spawner (footprint passes):
    // control +0x3C := the level's class for minion1, `Skill5` at T.
    let (mut w, _) = seeded(act_row(128, &nih), 2, |v| v[0] >= 40 && v[1] >= 75);
    give_skill(&mut w, 2, 380, 10);
    give_skill(&mut w, 5, 302, 4);
    w.fake.footprint = true;
    w.think_with(Some(w.player), 20, false);
    assert_eq!(w.store.control(w.mon).unwrap().spawn_class, 1453);
    assert_eq!(w.fake.modes(), [unit_mode(4, w.player)]);
    // Nothing else: draw < 60 → walk, then idle 5 (both).
    let (mut w, _) = seeded(act_row(128, &nih), 2, |v| v[0] >= 40 && v[1] < 60);
    w.think_with(Some(w.player), 20, false);
    assert_eq!(
        w.fake.modes(),
        [
            unit_mode(mode::WALK, w.player),
            unit_mode(mode::NEUTRAL, w.mon)
        ]
    );
    assert_eq!(w.thinks(), [5]);
}

// Covers: specs/monsters/ai-bodies-5.md §23 text
#[test]
fn nihlathak_alternate() {
    let mut w = world(act_row(128, &[]));
    let mon = w.mon;
    w.store.control_mut(mon).unwrap().function = 0x005E_5280;
    set_param_of(&mut w, 1, 3);
    w.fake.states.insert((mon, 12));
    w.think_with(None, 0, false);
    let c = w.store.control(mon).unwrap();
    assert_eq!((c.function, c.params), (0x005E_E5D0, [0; 3]));
    assert!(!w.fake.states.contains(&(mon, 12)));
    assert_eq!(w.thinks(), [1]);
}
