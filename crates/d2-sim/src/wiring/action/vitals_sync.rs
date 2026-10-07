// Spec: specs/combat/vitals.md §5.1–§5.3 (wiring of the client vitals sync)
//! The client vitals sync (`combat::vitals::sync`) on the action wiring:
//! the per-client caches (client +0x48C) live in [`SyncState`]; the
//! current values come from the unit's stat lists (totals, max life /
//! mana, the state 100 / 106 potion lists), its path record (position
//! and path target; `ActionHooks::paths`) or the host's staged position,
//! the charstats row of its class (`ActionHooks::vitals`) and the game
//! frame. The caller (the host's flush, `vitals.md` §5.1 rule 1) runs
//! [`run`] once per client at the end of each flush and sends what it
//! returns to that client.
//!
//! Opt-in per game, like the path provider: `ActionHooks::sync` is
//! `None` by default and [`ActionHooks::enable_vitals_sync`] turns it on.

use std::collections::BTreeMap;

use super::{ActionHooks, ActionSim};
use crate::combat::vitals::sync::{self, Current, ManaInputs, SyncCache};
use crate::game::Game;
use crate::stats::stat;
use crate::units::lists::client_state;
use crate::units::{ClientId, UnitId, UnitType};

/// `healthpot` / `manapot` list states (§5.2).
const STATE_HEALTHPOT: u32 = 100;
const STATE_MANAPOT: u32 = 106;
/// Stat 74 `hpregen`; 26 `manarecovery`; 27 `manarecoverybonus`; 13
/// `experience`; 14 `gold`; 6 / 8 / 10 life, mana, stamina.
const STAT_HPREGEN: u16 = 74;
const STAT_MANARECOVERY: u16 = 26;
const STAT_MANARECOVERYBONUS: u16 = 27;
const STAT_EXPERIENCE: u16 = 13;
const STAT_GOLD: u16 = 14;

/// The per-client caches of a game (client +0x48C, `0x00539330`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SyncState {
    // TODO(spec: vitals.md §5.2): the cache's values when the client
    // record is created are not written; read as all zero.
    pub caches: BTreeMap<ClientId, SyncCache>,
}

impl<X> ActionHooks<X> {
    /// Turns the client vitals sync on for this game.
    pub fn enable_vitals_sync(&mut self) {
        self.sync = Some(SyncState::default());
    }
}

/// The current values of §5.2 for `unit`. `staged`: the host's position
/// of a unit without a path record.
pub fn current<X>(sim: &ActionSim<X>, game: &Game, unit: UnitId, staged: (u16, u16)) -> Current {
    let s = &sim.sys.stats;
    let total = |k: u16| s.unit_total(unit, k, 0);
    let life_total = total(stat::HITPOINTS);
    let m = s.max_life(unit) >> 8;
    let path = sim.sys.hooks.paths.as_ref().and_then(|p| p.dynamic(unit));
    let (x, y, dx, dy) = match path {
        Some(p) => {
            let (x, y) = (p.x() as u16, p.y() as u16);
            (
                x,
                y,
                x.wrapping_sub(p.target_x) as u8,
                y.wrapping_sub(p.target_y) as u8,
            )
        }
        None => (staged.0, staged.1, 0, 0),
    };
    let list = s.unit_list(unit);
    let state_list = |st: u32| list.and_then(|r| s.list_of_state(r, st));
    let potion = state_list(STATE_HEALTHPOT).map(|l| (s.total(l, STAT_HPREGEN, 0), s.expire(l)));
    let lp = sync::life_prediction(potion, game.frame, life_total, m);
    let class = sim.sys.units.get(unit).map(|r| r.class);
    let mana_regen = sim
        .sys
        .hooks
        .vitals
        .as_ref()
        .zip(class)
        .and_then(|(v, c)| v.charstats(c as i32))
        .map(|c| c.manaregen);
    let mp = sync::mana_prediction(
        state_list(STATE_MANAPOT).map(|l| s.expire(l)),
        game.frame,
        &ManaInputs {
            max_mana: s.max_mana(unit),
            mana_regen,
            regen_pct: total(STAT_MANARECOVERYBONUS),
            regen_add: total(STAT_MANARECOVERY),
            mana_total: total(stat::MANA),
        },
    );
    Current {
        life: life_total >> 8,
        max_life: m,
        mana: total(stat::MANA) >> 8,
        stamina: total(stat::STAMINA) >> 8,
        lp,
        mp,
        x,
        y,
        dx,
        dy,
        gold: total(STAT_GOLD) as u32,
        exp: total(STAT_EXPERIENCE) as u32,
    }
}

/// One run of `0x0052D980` for `client` (§5.1 rules 1–3, §5.3): the
/// messages to send to that client, in order. `None` (nothing runs):
/// the sync is off, the client is not in game (state 4) or has no player
/// unit. `queued`: the client has a queued buffer (client +0x1B8,
/// §5.1 rule 2); `staged`: the host's position of a player without a
/// path record.
// TODO(spec: vitals.md §5.1 rule 3): the fatal assert of a client whose
// unit is not a player is read as "nothing runs".
// TODO(spec: vitals.md §5.1 rule 4, OQ7): `0x0052DA00` before the routine
// is not run.
pub fn run<X>(
    sim: &mut ActionSim<X>,
    game: &mut Game,
    client: ClientId,
    staged: (u16, u16),
    queued: bool,
) -> Option<Vec<Vec<u8>>> {
    sim.sys.hooks.sync.as_ref()?;
    let entry = game.lists.client(client)?;
    if entry.state != client_state::IN_GAME {
        return None;
    }
    let unit = entry.player?;
    if sim.sys.units.get(unit)?.ty != UnitType::Player {
        return None;
    }
    let force = sync::force(entry.update_count, queued);
    let now = current(sim, game, unit, staged);
    let caches = &mut sim.sys.hooks.sync.as_mut()?.caches;
    let r = sync::sync(caches.entry(client).or_default(), &now, force);
    if r.done {
        if let Some(e) = game.lists.client_mut(client) {
            e.update_count = 0;
        }
    }
    Some(r.messages)
}

#[cfg(test)]
mod tests;
