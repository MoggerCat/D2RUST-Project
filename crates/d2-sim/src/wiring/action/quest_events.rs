// Spec: specs/world/quests.md §4.3, §4.4 (kill parse), §4.6; specs/sim/tick.md §6 rule 5
//! The quest events the action wiring raises, queued for the host that
//! holds the quest control (the quest control is lent only to quest
//! calls): a monster kill (`0x00543A30`, [`QuestEvent::Kill`]) and a
//! player's level change (`0x00543B90`, [`QuestEvent::LevelChange`]).
//! [`ActionHooks::quest_events`] collects them in call order; the host
//! drains them with [`take`] after the call or tick that raised them.
//!
//! PROVISIONAL (M22; REC in `docs/HANDOFF.md` §7): the original runs the
//! kill parse inside the kill and the level change inside the client
//! update; here both run when the call or tick returns (the quest control
//! is the host's), and a level change's old level is the last level the
//! host saw for the player (0 before the first).

use crate::units::UnitId;

use super::{ActionHooks, Pending};

/// One queued quest event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuestEvent {
    /// `monster_killed(victim, killer)`.
    Kill { victim: UnitId, killer: UnitId },
    /// `changed_level(player, old, new)`.
    LevelChange { player: UnitId, old: u32, new: u32 },
}

/// Unit flag 0x80000000: no quest kill parse for this victim
/// (`quests.md` §4.4 step 3, `damage.md` §7.2).
pub const UNIT_FLAG_NO_QUEST_KILL: u32 = 0x8000_0000;

impl<X: Pending> ActionHooks<X> {
    /// Queues a kill for the quest control.
    pub fn queue_quest_kill(&mut self, victim: UnitId, killer: UnitId) {
        self.quest_events.push(QuestEvent::Kill { victim, killer });
    }

    /// Notes the player's level and queues the change when it differs
    /// from the last one noted (0 before the first).
    pub fn note_player_level(&mut self, player: UnitId, new: u32) {
        let old = self.quest_levels.insert(player, new).unwrap_or(0);
        if old != new {
            self.quest_events
                .push(QuestEvent::LevelChange { player, old, new });
        }
    }

    /// The queued events, in call order.
    pub fn take_quest_events(&mut self) -> Vec<QuestEvent> {
        std::mem::take(&mut self.quest_events)
    }
}

/// Andariel's monster class (`monstats.txt` row 156).
pub const ANDARIEL: u16 = 156;
/// A1Q6, Sisters to the Slaughter (`quests-act1.md` §10.8).
const A1Q6: u8 = 6;

/// Runs the queued events on the quest control.
///
/// PROVISIONAL (M22): the link of Andariel to chain 6 has no spec
/// (`quests.md` §4.6: "quest code also attaches links directly"; A1Q6's
/// init names none), so a kill of class [`ANDARIEL`] links it to chain 6
/// first. d2rs-own, unverified.
pub fn run<W: crate::world::quests::QuestWorld>(
    ctl: &mut crate::world::quests::QuestControl,
    w: &mut W,
    events: Vec<QuestEvent>,
) {
    for e in events {
        match e {
            QuestEvent::Kill { victim, killer } => {
                if w.monster_class(victim) == Some(ANDARIEL) {
                    ctl.add_link(w, victim, A1Q6, None);
                }
                ctl.monster_killed(w, victim, Some(killer));
            }
            QuestEvent::LevelChange { player, old, new } => ctl.changed_level(w, player, old, new),
        }
    }
}
