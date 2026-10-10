// Spec: specs/sim/intents-events.md §7.6 rule 5
//! The arena kill event `0x0053F720(game, killer, victim)` (called by the
//! kill, `damage.md` §7.2 step 2) and the per-client arena sync
//! `0x0053FC20` / clear `0x0053FAE0` of the client pass.
//!
//! A game has one arena record (game +0x1D28, flags at +0x08) and each
//! player an arena record (player data +0x34: a signed score at +0x00 and
//! a flag at +0x04). The kill adds the `arena.txt` row's column for the
//! kind of kill to the score, sets the flag, raises game flag 0x400 and
//! queues a unit for update; the same tick's sync sends S→C 0x65 to the
//! client whose player has the flag, and tick step 6 clears 0x400. The
//! score is never cleared. Row 0 is the game's arena type (every recorded
//! game: `Deathmatch`, `MonsterKill` 1).
//!
//! PROVISIONAL (REC-2810): the second loop of the sync `0x0053FB90` (0x65
//! of every other in-game player with the flag, to a client whose flag
//! `0x005388C0` is set) is not run; a single-player game has no other
//! player.

use std::collections::BTreeMap;

use crate::game::Game;
use crate::units::{UnitId, UnitType};

/// The arena state of a game; `None` in [`super::ActionHooks::arena`]
/// means the host does not model it (no event, no 0x65).
#[derive(Debug, Default, Clone)]
pub struct ArenaState {
    /// Game arena flag 0x400: a kill happened this tick.
    pub flag_400: bool,
    /// Per player: (score, flag).
    pub records: BTreeMap<UnitId, (i32, bool)>,
}

/// The `arena.txt` row columns, by their record offsets.
#[derive(Debug, Clone, Copy)]
pub struct ArenaRow {
    pub suicide: i32,
    pub player_kill: i32,
    pub player_kill_percent: i32,
    pub monster_kill: i32,
    pub player_death: i32,
    pub player_death_percent: i32,
    pub monster_death: i32,
}

impl From<&d2_data::tables::Arena> for ArenaRow {
    fn from(a: &d2_data::tables::Arena) -> Self {
        ArenaRow {
            suicide: a.suicide as i32,
            player_kill: a.playerkill as i32,
            player_kill_percent: a.playerkillpercent as i32,
            monster_kill: a.monsterkill as i32,
            player_death: a.playerdeath as i32,
            player_death_percent: a.playerdeathpercent as i32,
            monster_death: a.monsterdeath as i32,
        }
    }
}

impl ArenaState {
    fn credit(&mut self, unit: UnitId, delta: i32) {
        let r = self.records.entry(unit).or_default();
        r.0 = r.0.wrapping_add(delta);
        r.1 = true;
        self.flag_400 = true;
    }

    fn score(&self, unit: UnitId) -> i32 {
        self.records.get(&unit).map_or(0, |r| r.0)
    }

    /// `0x0053F720`: `killer` / `victim` with their unit types; `queue`
    /// is `0x0064C040`.
    pub fn kill_event(
        &mut self,
        row: &ArenaRow,
        game: &mut Game,
        killer: (UnitId, UnitType),
        victim: (UnitId, UnitType),
    ) {
        let (k, kt) = killer;
        let (v, vt) = victim;
        let mut queue = |u: UnitId| {
            let _ = game.lists.queue_update(u);
        };
        match (kt, vt) {
            (UnitType::Player, UnitType::Player) if k == v => {
                self.credit(k, row.suicide);
                queue(k);
            }
            (UnitType::Player, UnitType::Player) => {
                self.credit(k, row.player_kill);
                queue(v);
                let d = row.player_kill_percent.wrapping_mul(self.score(v)) / 100;
                self.credit(k, d);
                queue(v);
                self.credit(v, row.player_death);
                queue(k);
                let d = row.player_death_percent.wrapping_mul(self.score(k)) / 100;
                self.credit(v, d);
                queue(k);
            }
            (UnitType::Player, UnitType::Monster) => {
                self.credit(k, row.monster_kill);
                queue(v);
            }
            (UnitType::Monster, UnitType::Player) => {
                self.credit(v, row.monster_death);
                queue(k);
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ROW: ArenaRow = ArenaRow {
        suicide: -6,
        player_kill: 2,
        player_kill_percent: 10,
        monster_kill: 1,
        player_death: -2,
        player_death_percent: -10,
        monster_death: -1,
    };

    #[test]
    fn a_monster_kill_scores_the_row_and_raises_the_flag() {
        let mut st = ArenaState::default();
        let mut g = Game::new();
        let (p, m) = (UnitId(1), UnitId(2));
        st.kill_event(&ROW, &mut g, (p, UnitType::Player), (m, UnitType::Monster));
        assert_eq!(st.records[&p], (1, true));
        assert!(st.flag_400);
        st.kill_event(&ROW, &mut g, (p, UnitType::Player), (m, UnitType::Monster));
        assert_eq!(st.records[&p].0, 2);
    }

    #[test]
    fn a_player_kill_scores_both_sides_with_the_percent_columns() {
        let mut st = ArenaState::default();
        let mut g = Game::new();
        let (a, b) = (UnitId(1), UnitId(2));
        st.records.insert(b, (50, false));
        st.kill_event(&ROW, &mut g, (a, UnitType::Player), (b, UnitType::Player));
        // 2, then 10 * 50 / 100 = 5; victim -2, then -10 * 7 / 100 = 0.
        assert_eq!(st.records[&a].0, 7);
        assert_eq!(st.records[&b].0, 48);
    }

    #[test]
    fn other_pairs_do_nothing() {
        let mut st = ArenaState::default();
        let mut g = Game::new();
        st.kill_event(
            &ROW,
            &mut g,
            (UnitId(1), UnitType::Monster),
            (UnitId(2), UnitType::Monster),
        );
        assert!(!st.flag_400 && st.records.is_empty());
    }
}
