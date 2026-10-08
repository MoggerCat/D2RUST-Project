// Spec: specs/world/npc.md; specs/ui/npc-menus.tsv; docs/handoff/q-a3-town.md
//! The town NPCs of the synthetic play world, per act (d2rs-own,
//! unverified, REC-137): the (class, sub-tile offset from the town room
//! origin) pairs [`super::single_player::build_with_town`] allocates.
//! With game files the live town's NPCs come from its presets, and their
//! menus from `specs/ui/npc-menus.tsv`; this set only gives the synthetic
//! world the same classes so the NPC path can be driven headless.

use d2_sim::world::npc::class;

/// Act I: Akara and Kashya (the default synthetic town; Warriv is a
/// class only).
pub const ACT1: [(u16, i32); 2] = [
    (class::AKARA, super::single_player::AKARA_X),
    (class::KASHYA, super::single_player::KASHYA_X),
];

/// Natalya's `monstats` row (`npc-menus.tsv` record 20, Talk only).
pub const NATALYA: u16 = 251;

/// Act III (Kurast Docks): Ormus, Asheara, Hratli, Alkor, Natalya,
/// Meshif and Cain.
pub const ACT3: [(u16, i32); 7] = [
    (class::ORMUS, 34),
    (class::ASHEARA, 24),
    (class::HRATLI, 44),
    (class::ALKOR, 54),
    (NATALYA, 64),
    (class::MESHIF2, 74),
    (class::CAIN4, 84),
];

/// d2rs-own, unverified (REC-137): the synthetic game's `hireling` rows,
/// one per difficulty for Asheara (act 3, seller 252) so her hire list
/// can be made (`npc.md` §7.1 needs a row, `NoHirelingRow` otherwise).
/// The mercenary class, price, level and name ids are made up.
pub fn synthetic_hire_rows() -> Vec<d2_sim::world::npc::hire::HireRow> {
    let version = if super::single_player::GAME_SETUP.expansion {
        100
    } else {
        0
    };
    (1..=3)
        .map(|difficulty| d2_sim::world::npc::hire::HireRow {
            version,
            class: 357,
            act: 3,
            difficulty,
            seller: u32::from(class::ASHEARA),
            gold: 1000,
            level: 1,
            name_first: 3000,
            name_last: 3004,
        })
        .collect()
}
