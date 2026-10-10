// Spec: specs/sim/intents-events.md §7.6 rule 5 (0x65 kill count)
//! The arena kill record and its sync message.
//!
//! A player that kills a monster raises its arena record (`0x0053F720`,
//! branch `0x0053FA11`): count += the `arena.txt` row's `MonsterKill`, the
//! "seen" word := 1, and the game's arena flag bit 0x400 is set. The client
//! pass (`0x0053FC20`) then sends S→C 0x65 for the client's own player when
//! the bit is set and its record is seen; tick step 6 clears the bit
//! (`0x0053FAE0`), so there is one 0x65 per kill tick.

use std::collections::BTreeMap;

use crate::units::UnitId;

/// `arena.txt` has the one row `Deathmatch`, `MonsterKill` 1 (the game's
/// arena type is its only row). PROVISIONAL (REC-2820): the table is not
/// loaded into the sim.
pub const MONSTER_KILL: u16 = 1;

/// One player's arena record (player data +0x34): +0x00 the count, +0x04
/// the seen word.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ArenaRecord {
    pub count: u16,
    pub seen: bool,
}

/// The game's arena flag bit 0x400 and the players' records.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ArenaState {
    pub flag_0x400: bool,
    pub records: BTreeMap<UnitId, ArenaRecord>,
}

impl ArenaState {
    /// The kill event for a player killer of a monster (`0x0053F720`).
    pub fn monster_kill(&mut self, killer: UnitId) {
        let r = self.records.entry(killer).or_default();
        r.count = r.count.wrapping_add(MONSTER_KILL);
        r.seen = true;
        self.flag_0x400 = true;
    }

    /// The count to send for `player` this pass (`0x0053FC20` rule: bit
    /// 0x400 set and the record seen), if any.
    pub fn sync_count(&self, player: UnitId) -> Option<u16> {
        if !self.flag_0x400 {
            return None;
        }
        self.records
            .get(&player)
            .filter(|r| r.seen)
            .map(|r| r.count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Covers: specs/sim/intents-events.md §7.6 r5
    #[test]
    fn a_monster_kill_counts_once_per_flag_tick() {
        let mut a = ArenaState::default();
        let p = UnitId(1);
        assert_eq!(a.sync_count(p), None);
        a.monster_kill(p);
        assert_eq!(a.sync_count(p), Some(1));
        a.flag_0x400 = false;
        assert_eq!(a.sync_count(p), None);
        a.monster_kill(p);
        assert_eq!(a.sync_count(p), Some(2));
    }
}
