// Spec: specs/world/hirelings.md §1, §2; specs/world/npc.md §7.3
//! The synthetic game's hireling tables (d2rs-own, unverified; REC-157):
//! one `hireling` row per hire row of the play world (Kashya, Greiz,
//! Asheara, Qual-Kehk), so a hire in any act's town creates the
//! mercenary instead of stopping at `NoHirelingTables`. The stats,
//! growth and `Id`s are made up; the seller, act, difficulty, class,
//! price, level and name ids are the hire rows' own, so the offer the
//! hire list shows and the offer the init rolls come from the same row.

use d2_sim::world::hirelings::rows::{HirelingRow, HirelingRows};
use d2_sim::world::hirelings::HirelingTables;
use d2_sim::world::npc::HireRow;

/// The tables over `hire_rows` (one `Id` per row, in order from 1).
pub fn synthetic_hireling_tables(hire_rows: &[HireRow]) -> HirelingTables {
    let rows = hire_rows
        .iter()
        .enumerate()
        .map(|(i, h)| HirelingRow {
            version: h.version,
            id: i as u32 + 1,
            class: h.class,
            act: h.act,
            difficulty: h.difficulty,
            seller: h.seller,
            gold: h.gold as i32,
            level: h.level as i32,
            exp_lvl: 100,
            hp: 60,
            hp_lvl: 8,
            defense: 20,
            def_lvl: 4,
            str_: 30,
            str_lvl: 8,
            dex: 30,
            dex_lvl: 8,
            share: 10,
            dmg_min: 2,
            dmg_max: 6,
            dmg_lvl: 8,
            name_first: h.name_first,
            name_last: h.name_last,
            ..HirelingRow::default()
        })
        .collect();
    HirelingTables {
        rows: HirelingRows::new(rows),
        exp_ratios: Default::default(),
        max_level: 99,
        pet_flags: HirelingTables::WARP,
        pet_basemax: 1,
    }
}
