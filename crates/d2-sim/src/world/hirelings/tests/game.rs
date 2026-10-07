// Spec: specs/world/hirelings.md Test vectors (game files)
//! The live 1.14d `hireling`, `pettype` and `experience` tables
//! (`#[ignore]`, `D2_GAME_DIR`): the unit columns of the Test-vector
//! table (§4 at the row of §1.2 rule 2), the recorded hire of Diane
//! (Id 0, L 7) and the `ExpRatio` column of §7.2 rule 3.

use super::fake::Fake;
use crate::skills::tests_game as game;
use crate::units::UnitId;
use crate::world::hirelings::level::{apply_level, send_stats};
use crate::world::hirelings::{stat, HirelingState, HirelingTables, PetNode};

fn live() -> HirelingTables {
    HirelingTables::from_tables(
        &game::table("hireling", 280),
        &game::table("pettype", 224),
        &game::table("experience", 32),
    )
    .expect("live tables decode")
}

/// A level-99 player with a living hireling node of `id`; §4 at `level`.
fn applied(t: &HirelingTables, id: u32, level: i32) -> (Fake, UnitId, HirelingState, UnitId) {
    let mut w = Fake::new(true);
    let p = w.add(1, 0, 0, 1);
    let m = w.add(2, 1, 271, 13);
    w.set(p, stat::LEVEL, 99);
    let mut st = HirelingState::default();
    st.list_mut(p).nodes.push(PetNode {
        guid: 13,
        id,
        ..PetNode::default()
    });
    apply_level(&mut w, t, &st, p, Some(m), level);
    (w, p, st, m)
}

// Covers: specs/world/hirelings.md §4 r5, §4 r6, §4 r8
#[test]
#[ignore = "needs extracted 1.14d tables in D2_GAME_DIR"]
fn unit_columns_of_the_test_vectors() {
    let t = live();
    assert_eq!(t.max_level, 99);
    assert_eq!(t.pet_basemax, 1);
    // Id, L: str, dex, maxhp, def, min–max (23–24), tohit, resist,
    // hpregen, nextexp.
    let rows: [(u32, i32, [i32; 10]); 8] = [
        (1, 6, [38, 51, 18432, 39, 1, 3, 46, 6, 9, 41160]),
        (0, 2, [34, 43, 10240, 7, 1, 3, 0, 0, 5, 3600]),
        (6, 20, [76, 56, 72960, 166, 12, 19, 152, 40, 36, 1067220]),
        (15, 30, [67, 55, 75520, 155, 8, 14, 195, 51, 37, 3382720]),
        (24, 40, [123, 78, 129024, 300, 25, 29, 390, 77, 64, 8472240]),
        (1, 40, [82, 119, 105984, 339, 11, 13, 502, 73, 52, 7413210]),
        (
            1,
            70,
            [119, 179, 253440, 810, 27, 29, 1258, 124, 126, 38109960],
        ),
        (9, 50, [125, 97, 189952, 510, 27, 34, 553, 95, 94, 16230240]),
    ];
    for (id, l, want) in rows {
        let (w, _, _, m) = applied(&t, id, l);
        let got = [
            stat::STRENGTH,
            stat::DEXTERITY,
            stat::MAXHP,
            stat::ARMORCLASS,
            stat::SECONDARY_MINDAMAGE,
            stat::SECONDARY_MAXDAMAGE,
            stat::TOHIT,
            stat::FIRERESIST,
            stat::HPREGEN,
            stat::NEXTEXP,
        ]
        .map(|s| w.base(m, s));
        assert_eq!(got, want, "Id {id}, L {l}");
    }
}

// Covers: specs/world/hirelings.md §4, §13 r4
#[test]
#[ignore = "needs extracted 1.14d tables in D2_GAME_DIR"]
fn recorded_hire_of_diane_queues_the_recorded_stats() {
    // `merc1-spawn-packets.jsonl` frame 1731: Id 0, L 7; the 14 stats in
    // §13 rule 4 order with the recorded values.
    let t = live();
    let (mut w, p, st, m) = applied(&t, 0, 7);
    w.queued.clear();
    send_stats(&mut w, &st, p).unwrap();
    let want: Vec<(u16, u32)> = vec![
        (12, 7),
        (0, 40),
        (2, 53),
        (7, 0x5100),
        (6, 0x5100),
        (31, 47),
        (13, 39200),
        (30, 57600),
        (21, 2),
        (22, 4),
        (39, 8),
        (41, 8),
        (43, 8),
        (45, 8),
    ];
    assert_eq!(w.queued[&m], want);
}

// Covers: specs/world/hirelings.md §7.2 r3
#[test]
#[ignore = "needs extracted 1.14d tables in D2_GAME_DIR"]
fn live_exp_ratio_column() {
    let r = live().exp_ratios;
    assert_eq!((r.max_level, r.max_row), (99, 10));
    for l in 0..=69 {
        assert_eq!(r.ratio(l.max(1)), 1024, "level {l}");
    }
    assert_eq!(r.ratio(80), 496);
    assert_eq!(r.ratio(85), 256);
    let tail: Vec<i32> = (86..=99).map(|l| r.ratio(l)).collect();
    assert_eq!(
        tail,
        [192, 144, 108, 81, 61, 46, 35, 26, 20, 15, 11, 8, 6, 5]
    );
    assert_eq!(r.apply(3_000_000, 97), 23432);
}
