// Spec: specs/world/hirelings.md §6 r3–r4, §10; specs/world/hirelings-2.md §16, §19
//! The hireling callers of the wired host that the action wiring queues
//! ([`HirelingCall`] on `ActionHooks::hireling_calls`): the save restore
//! (`0x0056AA50`, `hirelings.md` §10 with `hirelings-2.md` §16), the join
//! follow (`0x005773D0`, §16 rule 3) and the act change's hireling part
//! (`0x0053ACC0`, `hirelings.md` §6 rules 3–4), run on this host's
//! hireling lists ([`WiredWorld::state`]) with the rules of
//! `d2_sim::world::hirelings::life`.
//!
//! TODO(hirelings-2.md §19): in 1.14d each call runs inside its caller
//! (the save load, the join placement, the level warp); here it runs when
//! the handler or tick that queued it returns (the hireling lists are
//! this host's, not the action wiring's), so its messages follow the
//! call's other messages. A restore queued by the load runs before the
//! join follow the entry queued (queue order). The classic act change's
//! `0x00575BC0` runs after the warp's placement (1.14d: before it).

use d2_sim::game::Game;
use d2_sim::items::inventory::UnitKind;
use d2_sim::units::lifecycle::AllocRequest;
use d2_sim::units::{UnitId, UnitType};
use d2_sim::wiring::action::HirelingCall;
use d2_sim::wiring::interaction::InteractionError;
use d2_sim::world::hirelings::life::{self, Loader, RestoreSkip, SavedHireling, MODE_DEAD};
use d2_sim::world::hirelings::{level, HirelingError};

use super::wired::{TradeRest, WiredWorld};
use super::ActionEvents;

/// The player's act as `0x005774F0` reads it (`hirelings.md` §10 rule
/// 3): the client act (`0x005382B0`: the act of the client's room) when
/// the player has a client, else the unit's act (+0x18: the act of its
/// room); neither → 0.
fn player_act(game: &Game, player: UnitId) -> u32 {
    let l = &game.lists;
    let client_room = l
        .clients()
        .into_iter()
        .filter_map(|c| l.client(c))
        .find(|e| e.player == Some(player))
        .and_then(|e| e.room);
    let room = client_room.or_else(|| l.unit(player).and_then(|e| e.room()));
    room.and_then(|r| l.room(r)).map_or(0, |r| u32::from(r.act))
}

impl<R: TradeRest, S> WiredWorld<R, S> {
    /// The hireling calls the action wiring queued, in order
    /// (`ActionHooks::hireling_calls`, on from the first frame). Without
    /// hireling tables the game has no hireling: the queue is dropped.
    /// A restore's skip is not reported (the queue's loader is
    /// [`Loader::Current`], whose skips are "no hireling"); a fatal skip
    /// goes to the interaction errors.
    pub fn hireling_calls<D: ActionEvents>(&mut self, game: &mut Game, events: &mut D) {
        let q = events
            .action()
            .sys
            .hooks
            .hireling_calls
            .as_mut()
            .map(std::mem::take)
            .unwrap_or_default();
        if q.is_empty() || self.state.hireling_tables.is_none() {
            return;
        }
        for call in q {
            match call {
                HirelingCall::Restore {
                    player,
                    saved,
                    loader,
                } => {
                    if let Err(RestoreSkip::Fatal(id)) =
                        self.restore_hireling(game, events, player, &saved, loader)
                    {
                        self.state
                            .errors
                            .push(InteractionError::Hireling(HirelingError::Fatal(id)));
                    }
                }
                HirelingCall::JoinFollow(p) => {
                    self.desk(game, events, |desk, _, _| {
                        desk.with_hirelings(|w, t, st| life::join_follow(w, t, st, p))
                    });
                }
                HirelingCall::ActChange(p) => {
                    self.desk(game, events, |desk, _, _| {
                        desk.with_hirelings(|w, t, st| {
                            // §6 rule 3: classic → `0x00575BC0` first; then
                            // the follow (`0x0053AEA6`) in both game types.
                            if !d2_sim::world::hirelings::HirelingWorld::expansion(w) {
                                life::classic_act_change(w, st, p);
                            }
                            life::follow(w, t, st, p);
                        })
                    });
                }
            }
        }
    }

    /// `0x0056AA50` (`hirelings.md` §10 rules 1–7, `hirelings-2.md` §16)
    /// for `player` with the saved hireling `saved`: the plan (rules 1–3,
    /// [`life::restore_plan`]); the hire slot of the name in the hire
    /// list of the seller's NPC record marked hired when it is not
    /// offered (rule 3); the unit of the plan's class and mode allocated
    /// **with no room** at (0, 0) (§16 rule 3); the init and the node's
    /// saved values ([`life::restore`]); experience and level (rules 5–6,
    /// [`level::restore_experience`], not on the version-0x47 path); the
    /// dead steps and the inventory in the loader's order (rule 7,
    /// [`life::restore_tail`]; the inventory is the inventory model's,
    /// [`WiredWorld::inventory`]). `Ok(None)`: no unit (no hireling
    /// tables, the allocation refused, or the level loop freed it). A
    /// living hireling gets a room at the join follow (§16 rule 3).
    pub fn restore_hireling<D: ActionEvents>(
        &mut self,
        game: &mut Game,
        events: &mut D,
        player: UnitId,
        saved: &SavedHireling,
        loader: Loader,
    ) -> Result<Option<UnitId>, RestoreSkip> {
        let Some(t) = self.state.hireling_tables.as_ref() else {
            return Ok(None);
        };
        let a = events.action();
        let expansion = a.sys.data.expansion;
        let difficulty = a.sys.hooks.ai_info.difficulty;
        let plan = life::restore_plan(
            t,
            expansion,
            difficulty,
            saved,
            player_act(game, player),
            loader,
        )?;
        // Rule 3: the slot of the name, only when it is not offered.
        // PROVISIONAL (hirelings.md §10 r3; REC-none): the slot is the
        // first slot of the list with that name id; a seller without an
        // NPC record or a hire list marks nothing.
        let slot = u16::try_from(plan.seller)
            .ok()
            .and_then(|c| self.npc.record_mut(c))
            .and_then(|r| r.hire.as_mut())
            .and_then(|h| h.slots.iter_mut().find(|s| s.name == plan.name));
        if let Some(s) = slot {
            if !s.offered {
                s.hired = true;
            }
        }
        // §16 rule 3: `0x00555230(1, class, 0, 0, game, room 0, flag 1,
        // mode, 0)`.
        let req = AllocRequest {
            ty: UnitType::Monster,
            class: plan.class,
            room: None,
            add: true,
            fixed_guid: None,
            mode: u32::from(plan.mode),
            allied: false,
        };
        // TODO(spec: hirelings.md §10 r3): a refused allocation is not
        // described; no hireling.
        let Some(merc) = events.action().with(game, |g, v| v.allocate(g, &req, 0, 0)) else {
            return Ok(None);
        };
        let dead = plan.mode == MODE_DEAD;
        let kept = self.desk(game, events, |desk, _, inv| {
            let r = desk.with_hirelings(|w, t, st| -> Result<bool, HirelingError> {
                life::restore(w, t, st, player, merc, &plan, saved)?;
                if loader != Loader::OldV47
                    && !level::restore_experience(
                        w,
                        t,
                        st,
                        player,
                        merc,
                        saved.experience,
                        loader.is_old(),
                    )?
                {
                    return Ok(false);
                }
                life::restore_tail(w, st, player, merc, dead, loader);
                Ok(true)
            });
            let kept = match r {
                Some(Ok(k)) => k,
                Some(Err(e)) => {
                    desk.state.errors.push(InteractionError::Hireling(e));
                    false
                }
                None => false,
            };
            // §16 rule 5: no inventory → `0x0063ABD0` on the inventory
            // model (the rest's `ensure_inventory` has none).
            if kept {
                if let (Some(inv), Some(guid)) = (inv, desk.econ.units.get(merc).map(|r| r.guid)) {
                    let kind = UnitKind::Monster { class: plan.class };
                    inv.state.add_inventory(merc, kind, guid);
                }
            }
            kept
        });
        Ok(kept.then_some(merc))
    }
}
