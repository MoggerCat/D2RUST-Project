// Spec: specs/tools/poke.md §2 r6, §5 r2 (pokes reach the server game through the bridge)
//! The poke tools' way to the server: a link whose server game can run a
//! poke directive (`d2_sim::poke`) between frames implements
//! [`PokeTarget`]; the bridge forwards the request, so tools reach the
//! game through the bridge like everything else (CLAUDE.md rule 5).

use d2_sim::poke::{PokeOp, PokeResult};

/// A link whose server game can run a poke between two frames.
pub trait PokeTarget {
    type Error;
    /// Runs `op` on the state after the last tick (references resolved
    /// there), before the next frame's drain.
    fn poke(&mut self, op: &PokeOp) -> Result<PokeResult, Self::Error>;
}

impl<L: PokeTarget> super::Bridge<L> {
    /// Runs `op` on the server game, between two frames.
    pub fn poke(&mut self, op: &PokeOp) -> Result<PokeResult, L::Error> {
        self.link.poke(op)
    }
}
