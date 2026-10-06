// Spec: specs/world/quests.md §7.3, §10.2; specs/world/npc.md §7.5
//! Quests → the NPC control block: the mercenary reward `0x00579180`
//! (`npc.md` §7.5, [`NpcControl::quest_mercenary`]) that an Act I quest
//! grants from C→S 0x31 (`quests.md` §10.2, Kashya's message 92).
//!
//! The quest code reaches the reward through its world
//! ([`crate::world::quests::QuestWorld::mercenary_reward`]) while it holds
//! the quest control block and the economy; the reward needs the NPC
//! control block and the desk as its world, which the quest call is
//! using. So [`Desk::quest_message`] collects the rewards during the
//! quest call and runs them on the NPC control block right after it,
//! before the message's result is returned.
//!
//! Order: the reward is the last action of chain 2's message-92 handler
//! (`act1::scroll`); of the records the list dispatch visits after it,
//! chains 3–6 have no message-92 case, but chain 37 (Act I intro) has an
//! event-11 function `0x0058F870` without a written body (it reaches
//! [`crate::wiring::economy::QuestRest::unhandled`]). In 1.14d that
//! function runs after the reward; here before it.
//!
//! TODO(quests.tsv chain 37, `npc.md` §7.5): the deferral is the same
//! order as 1.14d only if `0x0058F870` does nothing observable for
//! message 92 (open question in `docs/handoff/wire-open-seams.md`).
//!
//! A reward reached through another entry (no queue) still goes to
//! [`crate::wiring::economy::QuestRest::mercenary_reward`].

use crate::units::lifecycle::LifecycleHooks;
use crate::units::UnitId;
use crate::wiring::economy::{EconomyQuests, QuestRest};
use crate::world::npc::NpcControl;

use super::{Desk, InteractionError, NpcRest, PlayerQuestsRef};

impl<H: LifecycleHooks, R: NpcRest + QuestRest + PlayerQuestsRef> Desk<'_, '_, H, R> {
    /// C→S 0x31 (`quests.md` §7.3) with the mercenary rewards it grants
    /// run on `npc` (`npc.md` §7.5). Returns the quest handler's result
    /// code.
    pub fn quest_message(&mut self, npc: &mut NpcControl, player: UnitId, msg: &[u8]) -> u32 {
        let mut rewards = Vec::new();
        let code = {
            let mut w = EconomyQuests::new(self.econ, self.rest);
            w.mercenaries = Some(&mut rewards);
            self.quests.quest_message(&mut w, player, msg)
        };
        for (p, class) in rewards {
            if let Err(e) = npc.quest_mercenary(self, p, class) {
                self.state.errors.push(InteractionError::Npc(e));
            }
        }
        code
    }
}
