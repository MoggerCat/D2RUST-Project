// Spec: specs/world/npc.md §7.3 step 5; specs/ui/menus.md §3.3 (hire list rows)
//! The hire list's row stats for the play app ([`OriginalUi::set_hire_stats`]):
//! the offer of a slot is a pure function of its seed, so the client asks
//! the same `hire_init` (`0x006637F0`) the server ran, over the `hireling`
//! rows. The server prices the hire with these values, so the row's level
//! and cost are the ones it charges.
//!
//! d2rs-own, unverified: the list does not say which act's rows it was
//! rolled on, so the first act (1..5) whose picked row owns the name id
//! is used; the difficulty is Normal; Life and Def read 0 (no client
//! stat source for the mercenary, as `docs/handoff/stitch-hireling.md`).
//! PROVISIONAL: REC-HIRE-STATS (docs/HANDOFF.md §7).

use bevy::prelude::App;
use d2_sim::world::npc::{hire_init, HireRow};

use crate::ui::panels::npc_menu::HireStats;
use crate::world_view::WorldViewUi;

/// The stats of the offer `(name, seed)` for a player of `level`.
pub fn hire_stats(
    rows: &[HireRow],
    expansion: bool,
    name: u16,
    seed: u32,
    level: u32,
) -> Option<HireStats> {
    let version = if expansion { 100 } else { 0 };
    (0..5).find_map(|act| {
        let o = hire_init(rows, version, seed, act, 0, level)?;
        let r = rows.get(o.row)?;
        (r.name_first..=r.name_last)
            .contains(&name)
            .then_some(HireStats {
                level: o.level.max(0) as u32,
                hp: 0,
                ac: 0,
                cost: o.price,
            })
    })
}

/// Installs the stats closure over `rows` on the original UI; nothing
/// without it.
pub fn install_hire_stats(app: &mut App, rows: Vec<HireRow>, expansion: bool) {
    if let Some(mut ui) = app.world_mut().get_non_send_mut::<WorldViewUi>() {
        if let Some(o) = ui.original.as_mut() {
            o.set_hire_stats(Box::new(move |name, seed, level| {
                hire_stats(&rows, expansion, name, seed, level)
            }));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(act: u32, first: u16, last: u16, gold: u32) -> HireRow {
        HireRow {
            version: 100,
            class: 271,
            act,
            difficulty: 1,
            seller: 150,
            gold,
            level: 1,
            name_first: first,
            name_last: last,
        }
    }

    #[test]
    fn a_row_resolves_to_the_servers_offer() {
        let rows = [row(1, 100, 104, 100), row(2, 200, 204, 300)];
        let s = hire_stats(&rows, true, 202, 7, 10).expect("act 2 owns name 202");
        let o = hire_init(&rows, 100, 7, 1, 0, 10).unwrap();
        assert_eq!((s.level, s.cost), (o.level as u32, o.price));
        assert!(s.cost >= 300);
        // A name no row owns has no stats.
        assert_eq!(hire_stats(&rows, true, 999, 7, 10), None);
    }
}
