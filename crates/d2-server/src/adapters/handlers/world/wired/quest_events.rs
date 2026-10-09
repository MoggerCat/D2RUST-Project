// Spec: specs/world/quests.md §4.3 (dispatch), §4.4 (kill parse), §4.5 (level change), §4.6 (add link)
//! The quest events of the play host (tasks `q-a1-tower`, `q-a2-quests`): monster init's
//! chain links and monster deaths, which the action wiring's seams queue
//! ([`Pending::take_quest_events`]). They run on the quest control
//! after the tick (`after_tick`), as `0x005436B0` and `0x00543A30` do.
//! The players' level changes (`0x00543B90`, quest event 3) run in the
//! tick's per-client update (`sim/tick.md` §6 rule 5; the action
//! wiring's `client_level_change` on the lent quest control).
//!
//! PROVISIONAL (REC-129; Act II hooks REC-136): the original calls these from inside monster
//! init and the kill; here they run once per tick after the tick's
//! steps, in the order links, kills. `// d2rs-own, unverified`.

use d2_sim::game::Game;
use d2_sim::units::UnitType;
use d2_sim::wiring::action::{Pending, QuestEvent};
use d2_sim::world::quests::{act2, act5, QuestWorld};

use super::{quest_call, ActionEvents, TradeRest, WiredWorld};

/// Andariel's monster class (`monstats.txt` row 156).
const ANDARIEL: u16 = 156;
/// Mephisto's monster class (`monstats.txt` row 242, `quests-act3.md` §8).
/// Duriel's monster class (`monstats.txt` row 211).
const DURIEL: u16 = 211;
const MEPHISTO: u16 = d2_sim::world::quests::act3::npc::MEPHISTO;
/// Diablo's and Hephasto's monster classes (`quests-act4.md` §8; Hephasto's
/// base id is the class here, as `quests-act4.md` §4).
const DIABLO: u16 = 243;
const HEPHASTO: u16 = d2_sim::world::quests::act4::q3::HEPHASTO_BASE;

impl<R: TradeRest, S> WiredWorld<R, S> {
    /// Runs the queued quest events and the level changes since the last
    /// tick on the quest control.
    pub(super) fn run_quest_events<D: ActionEvents>(&mut self, game: &mut Game, events: &mut D) {
        let mut queued = events.action().sys.hooks.x.take_quest_events();
        // The cube's `hst ` hook (`0x0059E5C0`), recorded by its pending.
        if let Some(cube) = self.cube.as_mut() {
            for (player, code) in cube.pending.take_quest_items() {
                if code == *b"hst " {
                    queued.push(QuestEvent::StaffAssembled { player });
                }
            }
        }
        let frame = game.frame;
        let mut lair = None;
        let mut summit = None;
        self.desk(game, events, |desk, ctl, inv| {
            let ((), _) = quest_call(desk, ctl, inv, |q, w| {
                for e in &queued {
                    if let QuestEvent::Link { unit, chain } = *e {
                        q.add_link(w, unit, chain, None);
                    }
                }
                for e in &queued {
                    match *e {
                        QuestEvent::RadamentActivated { unit } => act2::q1::radament_ai(q, w, unit),
                        QuestEvent::SummonerActivated => act2::q5::summoner_seen(q, w),
                        QuestEvent::StaffAssembled { player } => {
                            act2::q2::staff_assembled(q, w, player)
                        }
                        QuestEvent::ShenkActivated { unit } => {
                            act5::q1::shenk_activated(q, w, unit)
                        }
                        QuestEvent::NihlathakActivated => act5::q4::nihlathak_ai_status(q, w),
                        QuestEvent::AncientsDisarm => act5::q5::disarm(q),
                        QuestEvent::BaalToStairs => act5::q6::chamber_open(q, w),
                        QuestEvent::AnyaOpenPortal { unit } => act5::q4::anya_ai_portal(q, w, unit),
                        // C→S 0x44 (REC-167): the staff in the orifice.
                        QuestEvent::InsertItem {
                            player,
                            object,
                            item,
                            action,
                        } => {
                            let item = w.unit_by_guid(UnitType::Item as u8, item);
                            act2::q6::item_to_object(q, w, player, object, item, action)
                        }
                        _ => {}
                    }
                }
                for e in &queued {
                    if let QuestEvent::Kill { victim, killer } = *e {
                        // PROVISIONAL (REC-132, d2rs-own, unverified): no
                        // spec links Andariel to chain 6.
                        if w.monster_class(victim) == Some(ANDARIEL) {
                            q.add_link(w, victim, 6, None);
                        }
                        // PROVISIONAL (REC-142, d2rs-own, unverified): the
                        // Guardian's link to Mephisto is by class, as
                        // Andariel's; no spec names where chain 20 is added.
                        if w.monster_class(victim) == Some(MEPHISTO) {
                            q.add_link(w, victim, 20, None);
                        }
                        // PROVISIONAL (REC-166, d2rs-own, unverified): the
                        // Act IV links of monster creation (`quests-act4.md`
                        // §8: 243 → chain 23, 409 → chain 24) by class, as
                        // Mephisto's; a refused duplicate is harmless.
                        // PROVISIONAL (REC-167, d2rs-own, unverified): Duriel's
                        // link to chain 13 is by class, as Andariel's.
                        match w.monster_class(victim) {
                            Some(DURIEL) => {
                                q.add_link(w, victim, 13, None);
                            }
                            Some(DIABLO) => {
                                q.add_link(w, victim, 23, None);
                            }
                            Some(HEPHASTO) => {
                                q.add_link(w, victim, 24, None);
                            }
                            _ => {}
                        }
                        q.monster_killed(w, victim, killer);
                    }
                }
                // Tick step 8 `0x00543E10`: the quest updater (timers such
                // as A1Q2's 15, `quests-act1.md` §10.5 r5) runs on every
                // 20th frame (`quests.md` §5; REC-130).
                if frame % 20 == 0 {
                    q.update(w);
                }
                lair = act2::q6::lair_warp_open(q);
                // PROVISIONAL (REC-246, d2rs-own, unverified): the exits close
                // only once the altar was used; the preview has no fight to
                // open them with, so a fresh game stays passable.
                summit = Some(act5::q5::summit_warp_open(q) || !act5::q5::altar_used(q));
            });
        });
        if let Some(open) = lair {
            events.action().sys.hooks.x.set_lair_open(open);
        }
        if let Some(open) = summit {
            events.action().sys.hooks.x.set_summit_open(open);
        }
    }
}
