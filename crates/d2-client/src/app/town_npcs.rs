// Spec: specs/world/npc.md; specs/ui/npc-menus.tsv; docs/handoff/q-a3-town.md
//! The town NPCs of the synthetic play world, per act (d2rs-own,
//! unverified, REC-137): the (class, sub-tile offset from the town room
//! origin) pairs [`super::single_player::build_with_town`] allocates.
//! With game files the live town's NPCs come from its presets, and their
//! menus from `specs/ui/npc-menus.tsv`; this set only gives the synthetic
//! world the same classes so the NPC path can be driven headless.

use d2_sim::world::npc::class;

/// Act I: Akara and Kashya (the default synthetic town; Warriv is a
/// class only), plus Gheed (gambles) and Charsi (repairs) east of Akara
/// (d2rs-own, unverified, REC-177: the real positions come from the
/// town presets). Act II's Elzix and Act IV's Jamella are in
/// `single_player::ACT2_NPCS` and `synthetic_act4::NPCS`.
pub const ACT1: [(u16, i32); 4] = [
    (class::AKARA, super::single_player::AKARA_X),
    (class::KASHYA, super::single_player::KASHYA_X),
    (class::GHEED, 36),
    (class::CHARSI, 8),
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

/// The Act III NPCs in the synthetic Kurast Docks room (d2rs-own,
/// unverified, REC-278: the real positions come from the town presets):
/// the [`ACT3`] classes in a row (x 3, 7, … at y 12, as Lut Gholein's),
/// inside the 40 × 40 sub-tile room; [`ACT3`]'s own offsets place them
/// beside the Act I start for the single-NPC rigs.
pub fn act3_docks() -> impl Iterator<Item = (u16, (i32, i32))> {
    ACT3.into_iter()
        .enumerate()
        .map(|(i, (class, _))| (class, (3 + 4 * i as i32, 12)))
}

/// Act V (Harrogath, REC-144; its own room, q-a5-town): Larzuk, Anya,
/// Malah, Nihlathak, Qual-Kehk and Cain.
pub const ACT5: [(u16, i32); 6] = [
    (class::LARZUK, 3),
    (class::DREHYA, 7),
    (class::MALAH, 11),
    (class::NIHLATHAK, 15),
    (class::QUAL_KEHK, 19),
    (class::CAIN6, 23),
];

/// d2rs-own, unverified (REC-137): the synthetic game's `hireling` rows,
/// one per difficulty for Asheara (act 3, seller 252) so her hire list
/// can be made (`npc.md` §7.1 needs a row, `NoHirelingRow` otherwise).
/// The mercenary class, level and name ids are made up; the price is 0
/// because the synthetic game has no gold stat row to pay from (REC-157).
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
            gold: 0,
            level: 1,
            name_first: 3000,
            name_last: 3004,
        })
        .chain((1..=3).map(|difficulty| d2_sim::world::npc::hire::HireRow {
            // Qual-Kehk (act 5, seller 515; REC-144): made-up barbarian
            // mercenary, price, level and names.
            version,
            class: 560,
            act: 5,
            difficulty,
            seller: u32::from(class::QUAL_KEHK),
            gold: 0,
            level: 1,
            name_first: 3100,
            name_last: 3102,
        }))
        .collect()
}
