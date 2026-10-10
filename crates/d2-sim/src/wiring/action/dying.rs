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
use super::{ActionHooks, ActionSim, Pending, View};
use crate::units::hooks::Sim;

/// Player modes 0 (DT) and 17 (DD).
const DT: u32 = 0;
const DD: u32 = 17;
/// The client mode-request codes of DT and DD (`client/model.md` §8 r4).
pub const CODE_DT: u8 = 8;
pub const CODE_DD: u8 = 9;
/// Neutral after death (`client/model.md` §8 r4 code 7).
pub const CODE_UP: u8 = 7;

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
        // The mode set `0x00624690(C, 17)` (`units.md` §4.1): a player's
        // allocation leaves mode 0, so 17 is a new mode and re-initialises
        // the animation fields (frame count +0x48, rate +0x4C). Recorded
        // 1.14d corpse: fc 256, sp 256 (gen-boss-708, frame 83).
        {
            let mut usim = crate::units::hooks::Sim {
                game: &mut *sim.game,
                units: &mut *v.units,
                stats: &mut *v.stats,
                data: v.data,
            };
            let _ = crate::units::modes::write_mode(&mut usim, &mut *v.h, c, DD);
        }
        v.set_state(c, STATE_PLAYERBODY as u16, true);
        // A player-type unit is good (`combat/hit.md` §7.1): the recorded
        // corpse's 0xAA names state 105 with stat 172 (REC-732).
        v.set_alignment(sim.game, c, 2);
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
    /// The death pass of one tick: tells the clients of each DT / DD
    /// change once (a lethal hit starts DT, [`Self::start_death`]; 0 life
    /// alone does not, `vitals.md` §4.8). Returns the players that
    /// changed.
    pub fn player_deaths(&mut self, game: &mut Game) -> Vec<UnitId> {
        let players = client_players(game);
        self.deaths_of(game, &players)
    }

    /// The DT start `0x00580EC0` for `p` now (the penalties, mode 0, the
    /// death animation): what a lethal hit requests (`damage.md` §7.1
    /// r5.4). A player already in DT / DD is left alone. The clients hear of it
    /// in the next [`Self::player_deaths`] pass.
    pub fn start_death(&mut self, game: &mut Game, p: UnitId) {
        self.with(game, |game, v| v.start_player_death(game, p, None));
    }

    /// [`Self::player_deaths`] for the given players (every one of them
    /// is told).
    pub fn deaths_of(&mut self, _game: &mut Game, players: &[UnitId]) -> Vec<UnitId> {
        let mut changed = Vec::new();
        // A corpse the pickup freed is no longer one.
        let units = &self.sys.units;
        self.sys
            .hooks
            .death
            .owners
            .retain(|c, _| units.get(*c).is_some_and(|r| r.mode == DD));
        // The corpses made since the last pass: the creation `0x0057F700`
        // broadcasts 0x8E (`0x0053DF80`: flag 1, the owner, the corpse) to
        // every client before anything else of the tick is sent. The unit
        // itself reaches the clients with its add messages in the client
        // pass (unit flag 0x10, `intents-events.md` §7.2), as recorded
        // (`items-drops-cha-00` frame 96).
        for c in s_fresh(self) {
            let s = &mut self.sys;
            let (Some(cguid), Some(owner)) = (
                s.units.get(c).map(|r| r.guid),
                s.hooks.death.owners.get(&c).copied(),
            ) else {
                continue;
            };
            let mut m = [0u8; 10];
            m[0] = 0x8E;
            m[1] = 1;
            m[2..6].copy_from_slice(&owner.to_le_bytes());
            m[6..10].copy_from_slice(&cguid.to_le_bytes());
            for &to in players {
                s.hooks.x.send(to, &m);
            }
        }
        for &p in players {
            let Some(mode) = self.sys.units.get(p).map(|r| r.mode) else {
                continue;
            };
            // No life check here: a player at 0 life stays alive until a
            // lethal hit requests DT (`vitals.md` §4.8 "Who starts them":
            // `damage.md` §7.1 r5.4 only). Recorded
            // `death-town-ama.check`: life poked to 0 at frame 10, the
            // player stays in TN with 0 life for 110 frames.
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
            // b = unit byte +0xB0 (`pathing.md` §10 rule 2: the DT / DD
            // rows); recorded 3 after a hit (`items-drops-cha-00`).
            let b = s.units.get(p).map_or(0, |r| r.hit_class as u8);
            let msg = crate::path::walk::messages::player_stop(
                UnitType::Player as u8,
                guid,
                code,
                x as u16,
                y as u16,
                b,
                0,
            );
            for &to in players {
                s.hooks.x.send(to, &msg);
            }
        }
        changed
    }
}

fn s_fresh<X>(a: &mut ActionSim<X>) -> Vec<UnitId> {
    std::mem::take(&mut a.sys.hooks.death.fresh)
}

impl<X: Pending> View<'_, X> {
    /// The DT start `0x00580EC0` for `p` with killer K (`vitals.md`
    /// §4.8): [`ActionSim::start_death`] and the lethal hit's request
    /// (`damage.md` §7.1 r5.4, K = the attacker).
    pub fn start_player_death(&mut self, game: &mut Game, p: UnitId, killer: Option<UnitId>) {
        if self
            .units
            .get(p)
            .is_none_or(|r| r.mode == DT || r.mode == DD)
        {
            return;
        }
        self.h.mode_target = killer;
        // Ours to announce (code 0: started, nothing sent yet).
        self.h.death.announced.entry(p).or_insert(0);
        self.h.death.died.insert(p);
        let mut sim = crate::units::hooks::Sim {
            game,
            units: &mut *self.units,
            stats: &mut *self.stats,
            data: self.data,
        };
        if let Err(e) = player_start(&mut sim, &mut *self.h, p, DT) {
            self.h.errors.push(super::WiringError::Unit(e));
        }
        self.h.mode_target = None;
    }
}

impl<X: Pending> ActionHooks<X> {
    /// `0x00580EC0` after the ear drop, with a killer K (the mode
    /// change's target): the death notice `0x0054D8A0` to every client and
    /// the arena kill event `0x0053F720` (`intents-events.md` §7.6 rule 6;
    /// 1.14d does not gate them on the ear drop).
    // PROVISIONAL (REC-2811): a minion killer is not resolved to its
    // player owner (`0x0058F0D0`); the record is a plain monster's.
    pub(super) fn death_notice(&mut self, sim: &mut Sim<'_>, victim: UnitId) {
        let Some(k) = self.mode_target else {
            return;
        };
        let (Some(kr), Some(vr)) = (sim.units.get(k), sim.units.get(victim)) else {
            return;
        };
        let (kty, kclass, vty) = (kr.ty, kr.class, vr.ty);
        let name_of = |h: &Self, u: UnitId| h.session.names.get(&u).copied().unwrap_or([0; 16]);
        let boss = (kty == UnitType::Monster)
            .then(|| self.monster_data(k))
            .flatten()
            .map(|d| (d.type_flags, d.boss_hc_idx));
        let mut killer_name = [0u8; 16];
        if kty == UnitType::Player {
            killer_name = name_of(self, k);
        }
        let m = crate::units::messages::death_notice(
            kclass,
            kty as u8,
            &name_of(self, victim),
            &killer_name,
            boss.filter(|b| b.0 & 2 != 0).map(|b| b.1),
        );
        for to in super::dying::client_players(sim.game) {
            self.x.send(to, &m);
        }
        if let (Some(row), Some(st)) = (
            self.tables.arena.first().map(super::arena::ArenaRow::from),
            self.arena.as_mut(),
        ) {
            st.kill_event(&row, sim.game, (k, kty), (victim, vty));
        }
    }
}
