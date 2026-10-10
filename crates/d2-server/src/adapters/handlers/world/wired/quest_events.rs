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
use d2_sim::world::quests::{act2, act3, act5, QuestWorld};

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
    /// The cube's `hst ` hook `0x0059E5C0` (`quests-act2.md` §4.9), run
    /// when the 0x4F handler returns: 1.14d calls it inside the
    /// transmute, so its S→C 0x28 follows the removal messages of the
    /// same handler, ahead of the next tick's item pass.
    pub(super) fn run_cube_staff<D: ActionEvents>(&mut self, game: &mut Game, events: &mut D) {
        let Some(cube) = self.cube.as_mut() else {
            return;
        };
        let players: Vec<_> = cube
            .pending
            .take_quest_items()
            .into_iter()
            .filter(|(_, code)| *code == *b"hst ")
            .map(|(player, _)| player)
            .collect();
        if players.is_empty() {
            return;
        }
        self.desk(game, events, |desk, ctl, inv| {
            let ((), _) = quest_call(desk, ctl, inv, |q, w| {
                for &player in &players {
                    act2::q2::staff_assembled(q, w, player);
                }
            });
        });
    }

    /// The kill parse `0x00543A30` (§4.4) of the monsters killed in the
    /// tick's unit steps, with the chain links queued before them, at the
    /// end of tick step 4 (before the client pass): 1.14d runs it inside
    /// the kill, after the treasure drop, so a quest drop (Radament's
    /// `ass `, Hephasto's `hfh `) is created and queued for update in the
    /// tick of the kill and announced in its client pass, ahead of the
    /// treasure drop's items (recorded: `items-drops-nor-09`, `-12`).
    /// The other queued events go back to the queue for
    /// [`Self::run_quest_events`] after the tick (PROVISIONAL, REC-129).
    pub(super) fn run_kill_events<D: ActionEvents>(&mut self, game: &mut Game, events: &mut D) {
        let queued = events.action().sys.hooks.x.take_quest_events();
        if !queued.iter().any(|e| matches!(e, QuestEvent::Kill { .. })) {
            for e in queued {
                events.action().sys.hooks.x.queue_quest_event(e);
            }
            return;
        }
        let (early, rest): (Vec<_>, Vec<_>) = queued
            .into_iter()
            .partition(|e| matches!(e, QuestEvent::Link { .. } | QuestEvent::Kill { .. }));
        for e in rest {
            events.action().sys.hooks.x.queue_quest_event(e);
        }
        self.desk(game, events, |desk, ctl, inv| {
            let ((), _) = quest_call(desk, ctl, inv, |q, w| {
                for e in &early {
                    if let QuestEvent::Link { unit, chain } = *e {
                        q.add_link(w, unit, chain, None);
                    }
                }
                for e in &early {
                    if let QuestEvent::Kill { victim, killer } = *e {
                        monster_killed(q, w, victim, killer);
                    }
                }
            });
        });
    }

    /// Runs the queued quest events and the level changes since the last
    /// tick on the quest control.
    pub(super) fn run_quest_events<D: ActionEvents>(&mut self, game: &mut Game, events: &mut D) {
        let mut queued = events.action().sys.hooks.x.take_quest_events();
        let picks = std::mem::take(&mut self.item_picks);
        // The cube's `hst ` hook (`0x0059E5C0`), recorded by its pending.
        if let Some(cube) = self.cube.as_mut() {
            for (player, code) in cube.pending.take_quest_items() {
                if code == *b"hst " {
                    queued.push(QuestEvent::StaffAssembled { player });
                }
            }
        }
        // The Golden Bird's boss choice (`quests-act3.md` §6.2) reads the
        // monstats flags byte +0x0D; only its bit 6 (`flying`, flag word
        // bit 14, `quests-act3-2.md` §11.4) is tested, so the byte is
        // built from that column. `None`: no monstats row.
        let bosses: Vec<(d2_sim::units::UnitId, u16, Option<u8>)> = {
            let sys = &events.action().sys;
            queued
                .iter()
                .filter_map(|e| match *e {
                    QuestEvent::BossCreated { unit } => Some(unit),
                    _ => None,
                })
                .filter_map(|u| {
                    let class = sys.units.get(u)?.class;
                    let row = sys.hooks.tables.combat.monstats.get(class as usize);
                    let flags = row.map(|m| if m.flying { act3::q4::FLYING_0D } else { 0 });
                    Some((u, u16::try_from(class).ok()?, flags))
                })
                .collect()
        };
        // `0x00545B50` jumps to a `ret` stub for units in levels ≥ 108
        // (`quests-act5-2.md` §7.9); below, to the council's `0x005BB550`
        // (`quests-act3.md` §7.5).
        let council: Vec<d2_sim::units::UnitId> = {
            let a = events.action();
            queued
                .iter()
                .filter_map(|e| match *e {
                    QuestEvent::PresetBoss { unit } => Some(unit),
                    _ => None,
                })
                .filter(|&u| {
                    let room = game.lists.unit(u).and_then(|e| e.room());
                    room.and_then(|r| a.sys.hooks.drlg.level_id(game, r))
                        .is_some_and(|l| l < 108)
                })
                .collect()
        };
        let frame = game.frame;
        let mut durance = None;
        let mut act3_npcs = None;
        let mut lair = None;
        let mut summit = None;
        let mut armed = false;
        let mut not_intro = Vec::new();
        let mut rescue = Vec::new();
        let mut quest_flags = Vec::new();
        self.desk(game, events, |desk, ctl, inv| {
            let ((), _) = quest_call(desk, ctl, inv, |q, w| {
                for e in &queued {
                    if let QuestEvent::Link { unit, chain } = *e {
                        q.add_link(w, unit, chain, None);
                    }
                }
                // Hook ITEMPICKEDUP `0x00543D80`: event 4 to the item's chain.
                for &(player, item) in &picks {
                    // Item creation `0x00555D20` links an item with `quest` ≠ 0
                    // to chain `quest` − 1 (`quests-act3.md` §10); linked here
                    // at its first pick-up (PROVISIONAL, REC-1556: the item
                    // creation hook is not wired).
                    let quest = w
                        .inner
                        .econ
                        .items
                        .get(item)
                        .and_then(|i| w.inner.econ.tables.item(i.record))
                        .map_or(0, |r| u32::from(r.quest));
                    if quest != 0 && w.quest_chain(item).is_none_or(|c| c.0.is_empty()) {
                        q.add_link(w, item, (quest - 1) as u8, None);
                    }
                    q.dispatch_chain(
                        w,
                        d2_sim::world::quests::event::ITEM_PICKED_UP,
                        d2_sim::world::quests::EventArgs {
                            player: Some(player),
                            target: Some(item),
                            ..Default::default()
                        },
                        false,
                    );
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
                        // REC-799: the prisoner AI's hooks (`quests-act5.md`
                        // §4.7, §4.10).
                        QuestEvent::WussieLeft { guid } => act5::q2::group_count_guid(q, guid),
                        QuestEvent::WussieRescue { player, unit } => {
                            act5::q2::rescue(q, w, player, unit)
                        }
                        QuestEvent::WussieWait => act5::q2::rescue_status(q, w),
                        // REC-796: Tyrael's spawn from the baalfx missile,
                        // at the missile's position of the call.
                        QuestEvent::SpawnTyrael {
                            room: Some(room),
                            x,
                            y,
                            ..
                        } => {
                            act5::q6::spawn_tyrael_at(w, room, x, y);
                        }
                        QuestEvent::AlkorReset => act3::alkor_bird_clear(q),
                        QuestEvent::OrmusAltar => act3::activate_altar(q, w),
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
                for &(unit, class, flags) in &bosses {
                    act3::choose_bird_boss(q, w, unit, class, flags);
                }
                for &unit in &council {
                    act3::council_preset(q, w, unit);
                }
                for e in &queued {
                    if let QuestEvent::Kill { victim, killer } = *e {
                        monster_killed(q, w, victim, killer);
                    }
                }
                // Tick step 8 `0x00543E10`: the quest updater (timers such
                // as A1Q2's 15, `quests-act1.md` §10.5 r5) runs on every
                // 20th frame (`quests.md` §5; REC-130).
                if frame % 20 == 0 {
                    q.update(w);
                }
                lair = act2::q6::lair_warp_open(q);
                // From any level but Durance 2 (the host tests the source).
                durance = Some(act3::durance_open(q, 0));
                act3_npcs = Some((act3::alkor_bird_brought(q), act3::altar_position(q)));
                // PROVISIONAL (REC-246, d2rs-own, unverified): the exits close
                // only once the altar was used; the preview has no fight to
                // open them with, so a fresh game stays passable.
                summit = Some(act5::q5::summit_warp_open(q) || !act5::q5::altar_used(q));
                armed = act5::q5::armed(q);
                not_intro = q.records.iter().map(|r| (r.chain, r.not_intro)).collect();
                rescue = act5::q2::barbarian_states(q);
                let d = usize::from(w.difficulty());
                for p in w.players() {
                    if let Some(f) = w.quests(p).map(|r| r.flags[d]) {
                        quest_flags.push((p, f));
                    }
                }
            });
        });
        if let Some(open) = lair {
            events.action().sys.hooks.x.set_lair_open(open);
        }
        if let Some((bird, altar)) = act3_npcs {
            events
                .action()
                .sys
                .hooks
                .x
                .set_act3_npc_answers(bird, altar);
        }
        if let Some(open) = durance {
            events.action().sys.hooks.x.set_durance_open(open);
        }
        events.action().sys.hooks.x.set_ancients_armed(armed);
        events
            .action()
            .sys
            .hooks
            .x
            .publish_quest_flags(&quest_flags);
        if let Some(open) = summit {
            events.action().sys.hooks.x.set_summit_open(open);
        }
        if !not_intro.is_empty() {
            events.action().sys.hooks.x.publish_not_intro(&not_intro);
            events.action().sys.hooks.x.publish_rescue(&rescue);
        }
    }
}

/// One queued kill: the by-class chain links of the bosses, then the
/// kill parse `0x00543A30` (§4.4).
fn monster_killed<W: QuestWorld>(
    q: &mut d2_sim::world::quests::QuestControl,
    w: &mut W,
    victim: d2_sim::units::UnitId,
    killer: Option<d2_sim::units::UnitId>,
) {
    // PROVISIONAL (REC-132, d2rs-own, unverified): no spec links Andariel
    // to chain 6.
    if w.monster_class(victim) == Some(ANDARIEL) {
        q.add_link(w, victim, 6, None);
    }
    // PROVISIONAL (REC-142, d2rs-own, unverified): the Guardian's link to
    // Mephisto is by class, as Andariel's; no spec names where chain 20 is
    // added.
    if w.monster_class(victim) == Some(MEPHISTO) {
        q.add_link(w, victim, 20, None);
    }
    // PROVISIONAL (REC-166, d2rs-own, unverified): the Act IV links of
    // monster creation (`quests-act4.md` §8: 243 → chain 23, 409 → chain
    // 24) by class, as Mephisto's; a refused duplicate is harmless.
    // PROVISIONAL (REC-167, d2rs-own, unverified): Duriel's link to chain
    // 13 is by class, as Andariel's.
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
        // The Ancients' chain-35 link is made at creation
        // (`quests-act5-2.md` §7.6; the superunique path).
        _ => {}
    }
    q.monster_killed(w, victim, killer);
}
