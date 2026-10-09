// Spec: specs/world/npc.md (§3), specs/world/vendors.md (§8)
//! Town and panel gaps of the play preview (q-town-gaps): the vendors a
//! player needs are in the synthetic towns.

use d2_client::app::single_player::{self, DEFAULT_SEED};
use d2_sim::units::UnitType;
use d2_sim::world::npc::class;

mod app_support;

/// Gheed and Charsi (act 1), Elzix (act 2) and Jamella (act 4) are
/// monster units of the synthetic game.
// Covers: specs/world/npc.md §3
#[test]
#[ignore = "real data: needs D2_GAME_DIR (tools/realdata-gate.sh)"]
fn the_gambler_and_repairers_of_the_preview_are_placed() {
    let mut g = single_player::build(&app_support::game_data(), DEFAULT_SEED).unwrap();
    let sim = &mut g.sim;
    let classes: Vec<u32> = sim
        .game
        .lists
        .units_of_type(UnitType::Monster)
        .into_iter()
        .filter_map(|u| {
            sim.events
                .action
                .with(&mut sim.game, |_, v| v.units.get(u).map(|r| r.class))
        })
        .collect();
    for c in [class::GHEED, class::CHARSI, class::ELZIX, class::JAMELLA] {
        assert!(classes.contains(&u32::from(c)), "class {c} is placed");
    }
}
