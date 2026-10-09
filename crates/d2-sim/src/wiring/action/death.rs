// Spec: specs/combat/vitals.md §4.6, §4.7; specs/sim/units.md §4.5
//! A player's death on the action wiring: the death penalties
//! `0x00535AB0` (`vitals.md` §4.6) from the DT start `0x00580EC0` (at
//! `0x00580F59`, [`crate::units::hooks::UnitHooks::player_death`]), the
//! corpse experience at corpse creation `0x0057F700` (from the DD start
//! `0x0057FCA0`, [`crate::units::hooks::UnitHooks::player_corpse`]) and
//! the corpse pickup `0x0057FB70` (§4.7 rule 2).
//!
//! The killer K is the DT start's unit target (`units.md` §4.5: the
//! unit form `0x00580A70` sets the target; `damage.md` §7.1 rule 5.4
//! requests DT with the attacker's type and GUID): the host that starts
//! DT puts it in [`ActionHooks::mode_target`].
//!
//! Client +0x508 ("experience lost", §4.7) is [`DeathState::exp_lost`],
//! keyed by the player (`0x005531C0`: one client per player).

// PROVISIONAL (sim/units.md §4.5): of `0x00580EC0` (before and after
// `0x00580F59`) and `0x0057FCA0` (character save) only the calls above
// run; settled by a bin read and a save capture on death (HIGH PRIORITY:
// saved bytes).

use std::collections::BTreeMap;

use crate::combat::vitals::experience::{
    corpse_experience, corpse_pickup, death_experience, gold_penalty,
};
use crate::combat::vitals::{stat, VitalsUnits};
use crate::units::hooks::Sim;
use crate::units::{UnitId, UnitType};

use super::combat::CombatView;
use super::{ActionHooks, Pending, View};

/// Stats of the gold penalty (§4.6 rule 1).
pub mod gold_stat {
    pub const GOLD: u16 = 14;
    pub const GOLDBANK: u16 = 15;
    pub const GOLDLOST: u16 = 175;
}

/// State 7 `playerbody` (§4.7 rule 2).
pub const STATE_PLAYERBODY: u32 = 7;

/// The gold limit `0x00622E70`: level × 10000 (`items/inventory-moves.md`
/// §7.22).
pub const GOLD_PER_LEVEL: i32 = 10_000;

/// The death state of the clients.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DeathState {
    /// Client +0x508 per player: the last death's experience loss
    /// (`0x005391E0` writes it, the corpse creation reads and clears it).
    pub exp_lost: BTreeMap<UnitId, u32>,
    /// The owner GUID of each corpse this wiring allocated (the
    /// inventory of a corpse, `0x0063D450`, is not modelled).
    pub owners: BTreeMap<UnitId, u32>,
    /// Allocate the corpse unit when [`Pending::create_corpse`] gives
    /// none (the preview host; off in the spec tests).
    pub allocate_corpses: bool,
    /// The last death code sent per player (8 DT, 9 DD): each goes out
    /// once ([`super::dying`]).
    pub announced: BTreeMap<UnitId, u8>,
    /// Players that have died: their dispatch gate follows the live mode
    /// (the host stages the others, `adapters::SimGame::set_player`).
    pub died: std::collections::BTreeSet<UnitId>,
    /// Corpses allocated and not yet announced to the clients.
    pub fresh: Vec<UnitId>,
    /// (player, corpse) pairs whose items have not moved yet: the host
    /// that holds the inventory model runs `items::moves::ground::
    /// corpse_fill` for each (`vitals.md` §4.7 rule 1.7).
    pub loot: Vec<(UnitId, UnitId)>,
    /// (player, amount) gold drops of the death penalty (`vitals.md` §4.6
    /// rule 1, `0x00535510`): the host that holds the inventory model
    /// makes the piles (`items::moves::ground::gold_piles`).
    pub gold_drops: Vec<(UnitId, i32)>,
}

impl<X: Pending> ActionHooks<X> {
    /// The death penalties `0x00535AB0(game, P, K)` (§4.6), K =
    /// [`Self::mode_target`].
    pub fn death_penalties(&mut self, sim: &mut Sim<'_>, p: UnitId) {
        let k = self.mode_target;
        let player_side = k.is_some_and(|k| self.player_side(sim, k));
        let pvp = k.is_some_and(|k| k != p) && player_side;
        let game_type = self.ai_info.game_type;
        let penalty = self
            .tables
            .combat
            .difficulty(usize::from(sim.data.difficulty))
            .map_or(0, |d| d.deathexppenalty);
        let vitals = self.vitals.clone();
        let mut cv = CombatView {
            game: &mut *sim.game,
            v: View::of(sim.units, sim.stats, sim.data, self),
        };
        // Rule 1: gold.
        let l = VitalsUnits::stat(&cv, p, stat::LEVEL);
        let gi = VitalsUnits::stat(&cv, p, gold_stat::GOLD);
        let gs = VitalsUnits::stat(&cv, p, gold_stat::GOLDBANK);
        let stash = cv.v.h.x.stash_cap(p);
        let g = gold_penalty(
            l,
            gi,
            gs,
            pvp,
            game_type,
            stash,
            l.wrapping_mul(GOLD_PER_LEVEL),
        );
        if let Some(v) = g.goldbank {
            cv.set_base_stat(p, gold_stat::GOLDBANK, v);
        }
        if pvp {
            if let Some(v) = g.gold {
                cv.set_base_stat(p, gold_stat::GOLD, v);
            }
            if let Some(q) = g.drop {
                cv.v.h.x.death_drop_gold(cv.game, p, q);
                cv.v.h.death.gold_drops.push((p, q));
            }
        } else {
            if let Some(q) = g.drop {
                cv.v.h.x.death_drop_gold(cv.game, p, q);
                cv.v.h.death.gold_drops.push((p, q));
            }
            if let Some(v) = g.gold {
                cv.set_base_stat(p, gold_stat::GOLD, v);
            }
        }
        cv.set_base_stat(p, gold_stat::GOLDLOST, g.goldlost);
        // Rule 2: experience, skipped for a player-side killer.
        if player_side {
            return;
        }
        let Some(t) = vitals else {
            return;
        };
        if let Some(loss) = death_experience(&mut cv, &t, p, penalty) {
            cv.v.h.death.exp_lost.insert(p, loss as u32);
        }
    }

    /// K a player, or a monster whose owner (`0x0058F0D0`) is a player.
    fn player_side(&self, sim: &Sim<'_>, k: UnitId) -> bool {
        let ty = |u: UnitId| sim.units.get(u).map(|r| r.ty);
        match ty(k) {
            Some(UnitType::Player) => true,
            Some(UnitType::Monster) => self
                .x
                .minion_owner(k)
                .is_some_and(|o| ty(o) == Some(UnitType::Player)),
            _ => false,
        }
    }

    /// Corpse creation `0x0057F700` (§4.7 rule 1): the corpse
    /// ([`Pending::create_corpse`]) gets stat 13 := `pct(v, 75, 100)` of
    /// client +0x508, then +0x508 := 0.
    /// With no corpse (§4.7 rule 1.2) +0x508 is left unchanged.
    pub fn corpse_creation(&mut self, sim: &mut Sim<'_>, p: UnitId) {
        let made = self.x.create_corpse(sim.game, p);
        let Some(c) = made.or_else(|| self.allocate_corpse(sim, p)) else {
            return;
        };
        let v = self.death.exp_lost.get(&p).copied().unwrap_or(0) as i32;
        let mut cv = CombatView {
            game: &mut *sim.game,
            v: View::of(sim.units, sim.stats, sim.data, self),
        };
        cv.set_base_stat(c, stat::EXPERIENCE, corpse_experience(v));
        cv.v.h.death.exp_lost.insert(p, 0);
    }

    /// Corpse pickup `0x0057FB70(game, P, C)` steps 1–2 (§4.7 rule 2,
    /// `inventory-moves.md` §12.1): C has state 7 (`playerbody`) and P
    /// may take it (`0x0057FAF0`: C's owner GUID is P's, or
    /// [`Pending::corpse_loot_allowed`]); its own player gets the corpse's
    /// experience back (the add §4.5). Returns the experience returned,
    /// or none when refused; the item take-back `0x00562F30` and the rest
    /// of §12.1 are the inventory's (`items::moves::ground`).
    pub fn corpse_pickup(&mut self, sim: &mut Sim<'_>, p: UnitId, c: UnitId) -> Option<i32> {
        // A corpse this wiring allocated has state 7 by construction (the
        // state table of a bare fixture may not hold the row).
        if !sim.stats.has_state(c, STATE_PLAYERBODY) && !self.death.owners.contains_key(&c) {
            return None;
        }
        let guid = sim.units.get(p).map(|r| r.guid);
        let owner = self
            .x
            .corpse_owner_guid(c)
            .or_else(|| self.death.owners.get(&c).copied());
        let own = owner.is_some() && owner == guid;
        if !own && !self.x.corpse_loot_allowed(c, p) {
            return None;
        }
        let mut x = 0;
        if let Some(t) = self.vitals.clone() {
            let mut cv = CombatView {
                game: &mut *sim.game,
                v: View::of(sim.units, sim.stats, sim.data, self),
            };
            x = corpse_pickup(&mut cv, &t, p, c, own);
        }
        Some(x)
    }
}
