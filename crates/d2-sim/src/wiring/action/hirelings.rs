// Spec: specs/world/hirelings-2.md §16 r3, §19; specs/world/hirelings.md §6 r3–r4, §10
//! The hireling calls the action wiring meets but cannot run: the
//! hireling lists live with the host that holds the interaction state
//! (`d2-server`'s `WiredWorld`), so the action wiring queues each call
//! ([`super::ActionHooks::hireling_calls`]) in the order it met it, and
//! the host runs the rules (`crate::world::hirelings::life`) on its
//! lists when the handler or tick that queued them returns.

use crate::units::UnitId;
use crate::world::hirelings::life::{Loader, SavedHireling};

/// One queued hireling call (`hirelings-2.md` §19 table).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HirelingCall {
    /// The save restore `0x0056AA50` (`hirelings.md` §10,
    /// `hirelings-2.md` §16) of `player`'s saved hireling, from the save
    /// load `0x0056B180` (game join).
    Restore {
        player: UnitId,
        saved: SavedHireling,
        loader: Loader,
    },
    /// The join follow `0x005773D0(game, player)` the join placement
    /// `0x005394A0` calls at `0x005396C3` (`hirelings-2.md` §16 rule 3).
    JoinFollow(UnitId),
    /// The act change `0x0053ACC0` of `player` (from the level warp
    /// `0x0053AEC0` to another act, `world/waypoints.md` §7 rule 5):
    /// classic → `0x00575BC0`, then the pet follow at `0x0053AEA6`
    /// (`hirelings.md` §6 rules 3–4).
    ActChange(UnitId),
}
