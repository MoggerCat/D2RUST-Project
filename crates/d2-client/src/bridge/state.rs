// Spec: specs/tools/state-snapshot.md §3 r1 (the d2rs snapshot point)
//! The debug state export's way to the server: a link that can take a
//! read-only snapshot of its game (`d2_sim::debug::state`) implements
//! [`StateSource`]; the bridge forwards the request, so tools reach the
//! game through the bridge like everything else (CLAUDE.md rule 5).

use d2_sim::debug::state::StateSnapshot;

/// A link whose server game can be snapshotted between frames.
pub trait StateSource {
    type Error;
    /// The game state after the last tick (frame = the game's frame).
    fn state_snapshot(&mut self) -> Result<StateSnapshot, Self::Error>;
}

impl<L: StateSource> super::Bridge<L> {
    /// The server game's state snapshot, between two frames.
    pub fn state_snapshot(&mut self) -> Result<StateSnapshot, L::Error> {
        self.link.state_snapshot()
    }
}
