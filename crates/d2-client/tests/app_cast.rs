// Spec: specs/skills/use.md (§3, §5, Test vectors: Fire Bolt L1), specs/sim/intents-events.md (§2.4 r4, §7.4)
//! A right-hand skill cast on the play host, headless, on the user's
//! install (`real_rig`; q-fixture-migrate): C→S 0x0C (the right skill at
//! a point) → the server's skill use → mana → the missile in the
//! server's store → the cast mode at the client. The skill is the
//! install's Fire Bolt, found by name in `skills.txt`; the mana it costs
//! is its own row's (`use.md` Test vectors: Fire Bolt L1).

mod app_support;
mod real_rig;

use d2_data::bin::TableFiles;
use d2_sim::units::UnitType;
use real_rig::{skill_row, Rig};

/// The install's skill id of `name` (`skills.txt`).
fn skill(name: &str) -> usize {
    let d = app_support::live();
    let (_, bytes) = d
        .archives
        .read_excel("skills.txt")
        .expect("skills.txt reads")
        .expect("the install has skills.txt");
    let text = String::from_utf8_lossy(&bytes).into_owned();
    let mut lines = text.lines();
    let head: Vec<&str> = lines.next().expect("header").split('\t').collect();
    let col = |n: &str| {
        head.iter()
            .position(|h| *h == n)
            .unwrap_or_else(|| panic!("{n}"))
    };
    let (c_name, c_id) = (col("skill"), col("Id"));
    for l in lines {
        let f: Vec<&str> = l.split('\t').collect();
        if f.get(c_name) == Some(&name) {
            return f[c_id].parse().expect("Id");
        }
    }
    panic!("no skills.txt row called {name}");
}

// Covers: specs/skills/use.md §5
#[test]
#[ignore = "real data: needs D2_GAME_DIR (tools/realdata-gate.sh)"]
fn a_right_skill_at_a_point_costs_mana_and_creates_the_missile() {
    let fire_bolt = skill("Fire Bolt");
    let row = skill_row(fire_bolt);
    let mut r = Rig::new("sorceress", &[fire_bolt]);
    r.leave_town();
    r.strengthen();
    r.select_right(fire_bolt);
    // Regeneration at rest, from half mana: the cast's fall is the cost
    // less what a frame regenerates.
    r.with(|sim, p| {
        sim.events
            .action
            .with(&mut sim.game, |_, v| v.set_base(p, 8, 50 << 8));
    });
    let (mut regen, mut prev) = (0, r.mana());
    for _ in 0..10 {
        r.step(1);
        let m = r.mana();
        regen = regen.max(m - prev);
        prev = m;
    }
    let (px, py) = r.pos();
    let mut msg = vec![0x0C];
    msg.extend(((px + 6) as u16).to_le_bytes());
    msg.extend((py as u16).to_le_bytes());
    let mark = r.s2c_mark();
    let fate = r.send_for_fate(&msg);
    assert!(fate.contains("Dispatched(Done)"), "{fate}");
    let (mut most, mut drop, mut modes) = (0, 0, Vec::new());
    for _ in 0..60 {
        r.step(1);
        most = most.max(r.with(|sim, _| sim.game.lists.units_of_type(UnitType::Missile).len()));
        let m = r.mana();
        drop = drop.max(prev - m);
        prev = m;
        modes.push(r.mode());
    }
    let errors = r.errors();
    // `use.md` Test vectors: Fire Bolt L1 pays `mana << manashift`.
    let cost = i32::from(row.mana) << row.manashift;
    assert!(
        (cost - regen..=cost).contains(&drop),
        "mana spent: drop {drop}, cost {cost}, regen {regen}; errors: {errors}"
    );
    assert!(most > 0, "the server made the missile; errors: {errors}");
    assert!(r.s2c_contains_since(mark, 0x4D), "{errors}");
    // The cast mode is the server player's. The client's own player
    // enters it at its click (`client/model.md` §20; `skills/sequences.md`
    // local player rule 1), not from a server message: 1.14d's server
    // sends its own client nothing (`docs/handoff/pc1-day4.md` item 55,
    // answered), and this rig sends the 0x0C bytes without a click.
    assert!(
        modes.contains(&u32::from(row.anim)),
        "the server player's cast mode: {modes:?}; {errors}"
    );
}
