// Spec: specs/world/hirelings.md §1, §2, §4 (tests on the live tables)
//! The Test-vector table of `hirelings.md` on the live 1.14d `hireling` /
//! `pettype` / `skills` tables (`D2_GAME_DIR`, CI skips it).

use super::fake::Fake;
use crate::world::hirelings::level::apply_level;
use crate::world::hirelings::{stat, HirelingState, HirelingTables, PetNode};
use d2_data::bin::BinTable;
use d2_data::tables::{decode_all, Hireling, Pettype, Record, Skills};

const PLAYER_GUID: u32 = 0x10;
const MERC_GUID: u32 = 0x4433_2211;
/// `experience` `MaxLvl` of class 0 in 1.14d (`hirelings.md` §4 rule 6).
const MAX_LEVEL: i32 = 99;

/// One extracted excel file as a table (tests may read game files).
#[allow(clippy::disallowed_methods)]
fn table(name: &str, size: usize) -> BinTable {
    let dir = std::env::var("D2_GAME_DIR").expect("D2_GAME_DIR must be set");
    let file = format!("{name}.bin");
    let path = format!("{dir}/extracted/patch_d2/data/global/excel/{file}");
    let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    BinTable::parse(name, "patch_d2.mpq", &file, &bytes, size).expect("table parses")
}
const INNER_SIGHT: u32 = 8;
const JAB: u32 = 10;
const COLD_ARROW: u32 = 11;
const PRAYER: u32 = 99;
const THORNS: u32 = 103;
const INFERNO: u32 = 41;
const FIRE_BALL: u32 = 47;
const BASH: u32 = 126;
const STUN: u32 = 139;

/// One vector row: the Id and level, the offer columns (life, str, dex,
/// price, exp, def, min, max, share, resist) and the unit columns (row, str,
/// dex, maxhp / 256, def, min, max, tohit, resist, hpregen, nextexp, skills).
struct Vector {
    id: u32,
    level: i32,
    offer: [i32; 10],
    row: usize, // bracket Level
    unit: [i32; 10],
    skills: &'static [(u32, i32)],
}

#[rustfmt::skip]
const VECTORS: [Vector; 8] = [
    Vector { id: 1, level: 6, offer: [72, 38, 51, 217, 26460, 39, 1, 3, 0, 0], row: 3,
        unit: [38, 51, 18432, 39, 1, 3, 46, 6, 9, 41160], skills: &[(INNER_SIGHT, 1), (COLD_ARROW, 1)] },
    Vector { id: 0, level: 2, offer: [40, 33, 43, 100, 1200, 7, 0, 2, 0, 0], row: 3,
        unit: [34, 43, 10240, 7, 1, 3, 0, 0, 5, 3600], skills: &[] },
    Vector { id: 6, level: 20, offer: [285, 76, 56, 927, 924000, 166, 12, 19, 1, 18], row: 9,
        unit: [76, 56, 72960, 166, 12, 19, 152, 40, 36, 1067220], skills: &[(JAB, 6), (PRAYER, 5)] },
    Vector { id: 15, level: 30, offer: [295, 67, 55, 3250, 3069000, 155, 8, 14, 0, 25], row: 15,
        unit: [67, 55, 75520, 155, 8, 14, 195, 51, 37, 3382720], skills: &[(INFERNO, 10), (FIRE_BALL, 8)] },
    Vector { id: 24, level: 40, offer: [504, 123, 78, 25200, 7872000, 300, 25, 29, 1, 56], row: 28,
        unit: [123, 78, 129024, 300, 25, 29, 390, 77, 64, 8472240], skills: &[(BASH, 7), (STUN, 6)] },
    Vector { id: 1, level: 40, offer: [378, 81, 119, 982, 6888000, 311, 10, 12, 0, 0], row: 36,
        unit: [82, 119, 105984, 339, 11, 13, 502, 73, 52, 7413210], skills: &[(INNER_SIGHT, 13), (COLD_ARROW, 13)] },
    Vector { id: 1, level: 70, offer: [648, 118, 179, 1657, 36529500, 551, 17, 19, 0, 0], row: 67,
        unit: [119, 179, 253440, 810, 27, 29, 1258, 124, 126, 38109960], skills: &[(INNER_SIGHT, 22), (COLD_ARROW, 22)] },
    Vector { id: 9, level: 50, offer: [742, 125, 97, 16195, 15300000, 510, 27, 34, 3, 83], row: 43,
        unit: [125, 97, 189952, 510, 27, 34, 553, 95, 94, 16230240], skills: &[(JAB, 15), (THORNS, 7)] },
];

// Spec: specs/world/hirelings.md Test vectors (the live-table rows)
// Covers: specs/world/hirelings.md §1.2 r2, §2, §4
#[test]
#[ignore = "needs extracted 1.14d tables in D2_GAME_DIR"]
fn test_vector_rows_on_the_live_tables() {
    let hireling = table(Hireling::TABLE, Hireling::SIZE);
    let pettype = table(Pettype::TABLE, Pettype::SIZE);
    let t = HirelingTables::from_tables(&hireling, &pettype, MAX_LEVEL).expect("tables");
    let skills: Vec<Skills> = decode_all(&table(Skills::TABLE, Skills::SIZE)).unwrap();
    for v in &VECTORS {
        let ctx = format!("Id {} L {}", v.id, v.level);
        // Offer: a seed that rolls this Id and level (Id 0 / 1 share Act 1).
        let first = &t.rows.rows[t.rows.row_at(true, v.id, 0).expect("row")];
        let (act0, diff0) = (first.act - 1, first.difficulty - 1);
        let offer = (0..1_000_000u32)
            .filter_map(|s| t.rows.offer(true, v.level + 2, s, act0, diff0))
            .find(|o| o.id == v.id && o.level == v.level)
            .unwrap_or_else(|| panic!("{ctx}: no seed rolls it"));
        let got = [
            offer.life,
            offer.strength,
            offer.dexterity,
            offer.price,
            offer.experience,
            offer.defense,
            offer.min_damage,
            offer.max_damage,
            offer.share,
            offer.resist,
        ];
        assert_eq!(got, v.offer, "{ctx}: offer");
        // Unit: the bracket of §1.2 rule 2 ("row N" = Level N) and §4.
        let at = t.rows.row_at(true, v.id, v.level).expect("row");
        assert_eq!(t.rows.rows[at].level, v.row as i32, "{ctx}: bracket");
        let mut w = Fake::new(true);
        w.skill_count = skills.len() as u32;
        for s in &skills {
            w.reqlevel.insert(u32::from(s.skill), s.reqlevel as i16);
        }
        let player = w.add(1, 0, 0, PLAYER_GUID);
        let merc = w.add(2, 1, 271, MERC_GUID);
        w.unit_mut(merc).owner = Some((PLAYER_GUID, 0));
        let mut st = HirelingState::default();
        st.list_mut(player).nodes.push(PetNode {
            guid: MERC_GUID,
            id: v.id,
            ..PetNode::default()
        });
        apply_level(&mut w, &t, &st, player, Some(merc), v.level);
        let b = |s| w.base(merc, s);
        let got = [
            b(stat::STRENGTH),
            b(stat::DEXTERITY),
            b(stat::MAXHP),
            b(stat::ARMORCLASS),
            b(stat::SECONDARY_MINDAMAGE),
            b(stat::SECONDARY_MAXDAMAGE),
            b(stat::TOHIT),
            b(stat::FIRERESIST),
            b(stat::HPREGEN),
            b(stat::NEXTEXP),
        ];
        assert_eq!(got, v.unit, "{ctx}: unit");
        let got: Vec<(u32, i32)> = w.unit(merc).skills.iter().map(|(k, l)| (*k, *l)).collect();
        let mut want = v.skills.to_vec();
        want.sort_unstable();
        assert_eq!(got, want, "{ctx}: skills");
    }
}
