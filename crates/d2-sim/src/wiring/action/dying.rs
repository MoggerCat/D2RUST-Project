// Spec: specs/combat/vitals.md §4.8; specs/combat/damage.md §7.1 r5.4; specs/sim/intents-events.md §9 r6
//! The player's death on the tick: a player whose life (stat 6) has
//! reached 0 starts the DT mode (`0x00580EC0` through
//! [`crate::units::modes::player_start`], which runs the penalties of
//! `vitals.md` §4.6), the ENDANIM event of the death animation turns it
//! into the DD start (`0x0057FCA0`: the corpse), and each change is told
//! to every client.
//!
//! PROVISIONAL (sim/pathing.md §10 r2: the death rows of the player
//! update function are not specified; REC-97): the S→C message is 0x0D
//! PlayerStop with the client's mode-request code 8 (DT) / 9 (DD)
//! (`client/model.md` §8 r4), the position and life percent 0. The
//! corpse is allocated here (a player-type unit of the class in mode 17
//! with state 7 `playerbody`, owner recorded in [`DeathState::owners`]);
//! its items stay with the player (`vitals.md` §4.7 r1.7 moves them;
//! the inventory model has no corpse grid).

use crate::game::Game;
use crate::units::lifecycle::AllocRequest;
use crate::units::modes::player_start;
use crate::units::{UnitId, UnitType};

use super::death::STATE_PLAYERBODY;
use super::{ActionSim, Pending, View};

/// Player modes 0 (DT) and 17 (DD).
const DT: u32 = 0;
const DD: u32 = 17;
/// The client mode-request codes of DT and DD (`client/model.md` §8 r4).
pub const CODE_DT: u8 = 8;
pub const CODE_DD: u8 = 9;
/// Neutral after death (`client/model.md` §8 r4 code 7).
pub const CODE_UP: u8 = 7;
/// Stat 6, `hitpoints`.
const LIFE: u16 = 6;

impl<X: Pending> super::ActionHooks<X> {
    /// The corpse unit of `p` (`vitals.md` §4.7 r1.3–1.4) when
    /// [`DeathState::allocate_corpses`](super::death::DeathState) is on.
    pub(super) fn allocate_corpse(
        &mut self,
        sim: &mut crate::units::hooks::Sim<'_>,
        p: UnitId,
    ) -> Option<UnitId> {
        if !self.death.allocate_corpses {
            return None;
        }
        let c = self.new_corpse(sim, p)?;
        self.death.fresh.push(c);
        self.death.loot.push((p, c));
        Some(c)
    }

    /// A player corpse unit of `p`'s class at `p`'s position, in its room
    /// (none for a player not yet placed): mode 17, state 7, owner `p`.
    fn new_corpse(&mut self, sim: &mut crate::units::hooks::Sim<'_>, p: UnitId) -> Option<UnitId> {
        let (x, y) = self.path_position(p);
        let r = sim.units.get(p)?;
        let (class, guid) = (r.class, r.guid);
        let room = sim.game.lists.unit(p).and_then(|e| e.room());
        let req = AllocRequest {
            ty: UnitType::Player,
            class,
            room,
            add: true,
            fixed_guid: None,
            mode: DD,
            allied: false,
        };
        let mut v = View::of(sim.units, sim.stats, sim.data, self);
        let c = v.allocate(sim.game, &req, x, y)?;
        // `vitals.md` §4.7 r1.4: C's seed from the game seed (`0x00552DF0`),
        // before its mode and state; a player allocation draws none
        // (`units.md` §3.1 r4.1).
        v.init_player_seed(c);
        v.set_state(c, STATE_PLAYERBODY as u16, true);
        if let Some(r) = v.units.get_mut(c) {
            r.mode = DD;
        }
        self.death.owners.insert(c, guid);
        Some(c)
    }
}

impl<X: Pending> ActionSim<X> {
    /// The corpse of a save (`formats/d2s.md` §8.3 rule 4, `0x0056A830`):
    /// a player corpse unit of `p`'s class linked to `p`, with no items
    /// (the caller reads the saved list into it). Not a death: no
    /// penalty, nothing moved from the player, nothing announced.
    /// PROVISIONAL (REC-282): where the original puts it is not specified;
    /// it is made where the player is at the join (no room yet).
    pub fn load_corpse(&mut self, game: &mut Game, p: UnitId) -> Option<UnitId> {
        let s = &mut self.sys;
        let mut sim = crate::units::hooks::Sim {
            game,
            units: &mut s.units,
            stats: &mut s.stats,
            data: &s.data,
        };
        s.hooks.new_corpse(&mut sim, p)
    }
}

/// The players of the game's clients, in list order.
pub fn client_players(game: &Game) -> Vec<UnitId> {
    game.lists
        .clients()
        .into_iter()
        .filter_map(|c| game.lists.client(c).and_then(|c| c.player))
        .collect()
}

impl<X: Pending> ActionSim<X> {
    /// The death pass of one tick: starts DT for each player with no
    /// life left and tells the clients of each DT / DD change once.
    /// Returns the players that changed.
    pub fn player_deaths(&mut self, game: &mut Game) -> Vec<UnitId> {
        let players = client_players(game);
        self.deaths_of(game, &players)
    }

    /// The DT start `0x00580EC0` for `p` now (the penalties, mode 0, the
    /// death animation): what a lethal hit requests (`damage.md` §7.1
    /// r5.4) and the life check of [`Self::player_deaths`] does. A
    /// player already in DT / DD is left alone. The clients hear of it
    /// in the next [`Self::player_deaths`] pass.
    pub fn start_death(&mut self, game: &mut Game, p: UnitId) {
        let s = &mut self.sys;
        if s.units.get(p).is_none_or(|r| r.mode == DT || r.mode == DD) {
            return;
        }
        s.hooks.mode_target = None;
        // Ours to announce (code 0: started, nothing sent yet).
        s.hooks.death.announced.entry(p).or_insert(0);
        s.hooks.death.died.insert(p);
        let mut sim = crate::units::hooks::Sim {
            game,
            units: &mut s.units,
            stats: &mut s.stats,
            data: &s.data,
        };
        if let Err(e) = player_start(&mut sim, &mut s.hooks, p, DT) {
            s.hooks.errors.push(super::WiringError::Unit(e));
        }
    }

    /// [`Self::player_deaths`] for the given players (every one of them
    /// is told).
    pub fn deaths_of(&mut self, game: &mut Game, players: &[UnitId]) -> Vec<UnitId> {
        let mut changed = Vec::new();
        // A corpse the pickup freed is no longer one.
        let units = &self.sys.units;
        self.sys
            .hooks
            .death
            .owners
            .retain(|c, _| units.get(*c).is_some_and(|r| r.mode == DD));
        for &p in players {
            let Some(mode) = self.sys.units.get(p).map(|r| r.mode) else {
                continue;
            };
            if self.sys.stats.unit_total(p, LIFE, 0) > 0 {
                self.sys.hooks.death.seen_alive.insert(p);
            } else if mode != DT && mode != DD && self.sys.hooks.death.seen_alive.remove(&p) {
                self.start_death(game, p);
            }
            let s = &mut self.sys;
            let mode = s.units.get(p).map_or(mode, |r| r.mode);
            let code = match mode {
                DT => CODE_DT,
                DD => CODE_DD,
                _ => {
                    // Back on its feet (0x41): code 7, neutral after being
                    // dead (`client/model.md` §8 r4).
                    if s.hooks.death.announced.remove(&p).is_some() {
                        changed.push(p);
                        let Some(guid) = s.units.get(p).map(|r| r.guid) else {
                            continue;
                        };
                        let (x, y) = s.hooks.path_position(p);
                        let msg = crate::path::walk::messages::player_stop(
                            UnitType::Player as u8,
                            guid,
                            CODE_UP,
                            x as u16,
                            y as u16,
                            0,
                            100,
                        );
                        for &to in players {
                            s.hooks.x.send(to, &msg);
                        }
                    }
                    continue;
                }
            };
            // A unit that is in mode 0 / 17 without our start (a bare
            // fixture) is not announced.
            match s.hooks.death.announced.get(&p) {
                Some(&c) if c != code => {}
                _ => continue,
            }
            s.hooks.death.announced.insert(p, code);
            changed.push(p);
            let Some(guid) = s.units.get(p).map(|r| r.guid) else {
                continue;
            };
            let (x, y) = s.hooks.path_position(p);
            let msg = crate::path::walk::messages::player_stop(
                UnitType::Player as u8,
                guid,
                code,
                x as u16,
                y as u16,
                0,
                0,
            );
            for &to in players {
                s.hooks.x.send(to, &msg);
            }
        }
        // The corpses of this pass: 0x59 (the unit appears), then 0x0D
        // code 9 (it lies dead).
        let fresh = s_fresh(self);
        for c in fresh {
            let s = &mut self.sys;
            let Some(r) = s.units.get(c) else { continue };
            let (class, cguid) = (r.class as u8, r.guid);
            let owner = s.hooks.death.owners.get(&c).copied();
            let name = players
                .iter()
                .find(|&&u| s.units.get(u).map(|r| r.guid) == owner)
                .and_then(|u| s.hooks.session.names.get(u).copied())
                .unwrap_or_default();
            let (x, y) = s.hooks.path_position(c);
            let add = super::switch::assign_player(cguid, class, &name, x as u16, y as u16);
            let dead = crate::path::walk::messages::player_stop(
                UnitType::Player as u8,
                cguid,
                CODE_DD,
                x as u16,
                y as u16,
                0,
                0,
            );
            for &to in players {
                s.hooks.x.send(to, &add);
                s.hooks.x.send(to, &dead);
            }
            // Announced here: the client pass that follows in the same
            // tick (this pass runs at the end of step 4) sends it no add
            // messages of its own (unit flag 0x10, `intents-events.md`
            // §7.1 rule 2.1), which would re-create it out of mode 17.
            if let Some(r) = s.units.get_mut(c) {
                r.flags &= !crate::units::record::flags::SEED_SET;
            }
        }
        changed
    }
}

fn s_fresh<X>(a: &mut ActionSim<X>) -> Vec<UnitId> {
    std::mem::take(&mut a.sys.hooks.death.fresh)
}
