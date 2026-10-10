// Spec: specs/sim/pets.md §2–§8; specs/skills/bodies.md §6.2; specs/monsters/umod-callbacks.md §1 rule 5
//! Summoned units on the skill pipeline's real providers: the summon
//! spawn's monster creation (`0x005B2F20`, [`BodyWorld::create_monster`])
//! and its pet list add ([`BodyEffect::PetAdd`], `0x00575D90`) run here
//! when [`Pending`] has no provider of its own. The lists are
//! [`ActionHooks::pet_lists`] (player data +0x44), the rules
//! [`crate::player::pets`] over [`PetView`].
//!
//! d2rs-own, unverified (preview): a summon is placed on the aimed point
//! itself (the original's spread search `0x005B2F20` is a path-spec
//! seam); a pet that dies or vanishes leaves its list on the next
//! [`UseView::pet_sweep`]. The lists' maxima resync `0x00575900`
//! ([`UseView::resync_pet_maxima`]) runs the skills' `petmax` calcs.
//! PROVISIONAL (pets.md OQ4): the add record's +0x10 / +0x14 stay 0.

use crate::game::Game;
use crate::player::pets::{self, PetLists, PetMsg, PetWorld};
use crate::skills::use_::bodies::BodyEffect;
use crate::units::lifecycle::AllocRequest;
use crate::units::modes::monster_mode;
use crate::units::{RoomId, UnitId, UnitType};
use crate::wiring::action::{reaction, Pending, View};

use super::skill_use::UseView;
use super::UseRest;

/// Monster mode 0 request (death), `0x005A7C20`.
const DEATH_REQUEST: i32 = 0;

/// [`PetWorld`] over the skill pipeline's view.
pub struct PetView<'v, 'a, X> {
    pub u: &'v mut UseView<'a, X>,
}

impl<X: Pending + UseRest> PetWorld for PetView<'_, '_, X> {
    type Unit = UnitId;

    fn is_player(&self, u: UnitId) -> bool {
        self.u
            .cv
            .v
            .units
            .get(u)
            .is_some_and(|r| r.ty == UnitType::Player)
    }
    fn has_player_data(&self, player: UnitId) -> bool {
        self.is_player(player)
    }
    fn pet_lists(&mut self, player: UnitId) -> Option<&mut PetLists> {
        let n = usize::try_from(self.pettype_count()).unwrap_or(0);
        Some(
            self.u
                .cv
                .v
                .h
                .pet_lists
                .entry(player)
                .or_insert_with(|| PetLists::new(n)),
        )
    }
    fn pettype_count(&self) -> i32 {
        self.u
            .cv
            .v
            .h
            .bodies
            .as_deref()
            .map_or(0, |b| b.pettype_count)
    }
    fn pettype_group(&self, t: i32) -> i16 {
        let b = self.u.cv.v.h.bodies.as_deref();
        usize::try_from(t)
            .ok()
            .and_then(|t| b?.pettype_group.get(t).copied())
            .unwrap_or(0)
    }
    fn monster_by_guid(&self, guid: i32) -> Option<UnitId> {
        self.u
            .cv
            .game
            .lists
            .find_unit(UnitType::Monster, guid as u32)
    }
    fn unit_guid(&self, u: UnitId) -> i32 {
        self.u.cv.v.units.get(u).map_or(-1, |r| r.guid as i32)
    }
    fn unit_class(&self, u: UnitId) -> u16 {
        self.u.cv.v.units.get(u).map_or(0, |r| r.class as u16)
    }
    fn flag_c8_bit8(&self, u: UnitId) -> bool {
        self.u
            .cv
            .v
            .units
            .get(u)
            .is_some_and(|r| r.flags2 & 0x100 != 0)
    }
    fn unit_flags(&self, u: UnitId) -> u32 {
        self.u.cv.v.units.get(u).map_or(0, |r| r.flags)
    }
    fn set_unit_flags(&mut self, u: UnitId, flags: u32) {
        if let Some(r) = self.u.cv.v.units.get_mut(u) {
            r.flags = flags;
        }
    }
    fn killable(&self, u: UnitId) -> bool {
        let class = self.unit_class(u) as usize;
        self.u
            .cv
            .v
            .h
            .tables
            .combat
            .monstats
            .get(class)
            .is_some_and(|m| m.killable)
    }
    fn kill(&mut self, u: UnitId) {
        let by = self.owner(u).unwrap_or(u);
        reaction::kill(&mut self.u.cv, u, by);
    }
    fn owner(&self, u: UnitId) -> Option<UnitId> {
        let guid = self.unit_guid(u);
        self.u
            .cv
            .v
            .h
            .pet_lists
            .iter()
            .find(|(_, l)| {
                l.entries
                    .iter()
                    .any(|e| e.nodes.iter().any(|n| n.guid == guid))
            })
            .map(|(&p, _)| p)
    }
    fn request_death_mode(&mut self, u: UnitId, target: Option<UnitId>) {
        Pending::mode_request(self.u.xm(), u, DEATH_REQUEST, target);
    }
    /// `0x00575900` (`sim/pets.md` §10 "Resync", OQ2 answered).
    fn resync(&mut self, player: UnitId) {
        self.u.resync_pet_maxima(player);
    }
    fn players(&self) -> Vec<UnitId> {
        self.u.cv.game.lists.units_of_type(UnitType::Player)
    }
    fn has_state(&self, u: UnitId, state: u16) -> bool {
        self.u.cv.v.stats.has_state(u, u32::from(state))
    }
    fn send(&mut self, client_player: UnitId, msg: PetMsg) {
        if let Some(b) = msg.bytes() {
            Pending::send(self.u.xm(), client_player, &b);
        }
    }
}

impl<X: Pending + UseRest> UseView<'_, X> {
    /// The pet-maximum resync `0x00575900(game, player)` (`sim/pets.md`
    /// §10): [`crate::skills::use_::bodies::pet_resync`] over the
    /// player's skill list with `pettype` `basemax` and
    /// [`pets::set_max`]. A player without pet lists is left alone.
    pub(super) fn resync_pet_maxima(&mut self, player: UnitId) {
        if !self.cv.v.h.pet_lists.contains_key(&player) {
            return;
        }
        let t = self.cv.v.h.tables.clone();
        let base: Vec<i16> = self
            .cv
            .v
            .h
            .bodies
            .as_deref()
            .map(|b| b.pettype_basemax.clone())
            .unwrap_or_default();
        let basemax = |pt: i32| {
            usize::try_from(pt)
                .ok()
                .and_then(|i| base.get(i))
                .map(|&b| i32::from(b))
        };
        crate::skills::use_::bodies::pet_resync(
            self,
            &t.skills,
            player,
            &basemax,
            &mut |w, pt, v| {
                if let Err(err) = pets::set_max(&mut PetView { u: w }, player, pt, v) {
                    w.cv.v
                        .h
                        .errors
                        .push(crate::wiring::action::WiringError::Pet(err));
                }
            },
        );
    }

    /// `0x005B2F20` on the allocator: a monster of `class` in `room` at
    /// the subtile `at`, in mode `mode` (module docs).
    pub(super) fn alloc_monster(
        &mut self,
        room: RoomId,
        at: (i32, i32),
        class: i32,
        mode: i32,
    ) -> Option<UnitId> {
        let req = AllocRequest {
            ty: UnitType::Monster,
            class: u32::try_from(class).ok()?,
            room: Some(room),
            add: true,
            fixed_guid: None,
            mode: u32::try_from(mode).unwrap_or(0),
            allied: false,
        };
        let game = &mut *self.cv.game;
        self.cv.v.allocate(game, &req, at.0, at.1)
    }

    /// [`BodyEffect::PetAdd`] on the lists, [`BodyEffect::OwnerData`] on
    /// the AI control and [`BodyEffect::SetSkill`] on
    /// [`crate::wiring::action::ActionHooks::monster_skills`]; every other
    /// effect (and the seam parts of these two) comes back for
    /// [`Pending::body_effect`].
    pub(super) fn pet_effect(
        &mut self,
        e: BodyEffect<UnitId, UnitId, RoomId>,
    ) -> Option<BodyEffect<UnitId, UnitId, RoomId>> {
        if let BodyEffect::SetSkill { m, skill, lvl } = e {
            // `0x0056DEB0` (`bodies.md` §6.5 step 6) on a monster: its
            // entry's base level; the refreshes go on to the seam.
            if self
                .cv
                .v
                .units
                .get(m)
                .is_some_and(|r| r.ty == UnitType::Monster)
            {
                self.cv
                    .v
                    .h
                    .monster_skills
                    .entry(m)
                    .or_default()
                    .insert(skill, lvl);
            }
            return Some(e);
        }
        if let BodyEffect::SourceFields { m, owner } = e {
            // `0x00621C30`: +0x94 / +0x98 := the owner's type and GUID
            // (0 / 0 for none).
            let link = owner.and_then(|o| {
                let ty = self.cv.v.units.get(o)?.ty;
                let guid = self.cv.game.lists.unit(o)?.guid;
                Some((ty.index() as u32, guid))
            });
            if let Some(r) = self.cv.v.units.get_mut(m) {
                r.source = link.unwrap_or((0, 0));
            }
            return None;
        }
        if let BodyEffect::WaitThink { m, frames } = e {
            // "wait N" `0x005DE0F0(game, m, N)` (`ai.md` §1.2 table):
            // delete the thinks and schedule one at frame + N, the mode
            // unchanged; without the AI store the seam's.
            if self.cv.v.h.ai.is_none() {
                return Some(e);
            }
            let v = &mut self.cv.v;
            let mut sim = crate::units::hooks::Sim {
                game: &mut *self.cv.game,
                units: &mut *v.units,
                stats: &mut *v.stats,
                data: v.data,
            };
            v.h.with_ai(&mut sim, |g, cx| {
                crate::monsters::ai::idle_keep_mode(g, cx, m, frames)
            });
            return None;
        }
        if let BodyEffect::OwnerData { m, owner, a, b } = e {
            self.owner_data(m, owner);
            // f1 / f2 ≠ 0 also restart the AI (`0x005DD230`): the seam's.
            return (a != 0 || b != 0).then_some(e);
        }
        let BodyEffect::PetAdd { owner, pet, t, max } = e else {
            return Some(e);
        };
        if let Err(err) = pets::add(&mut PetView { u: self }, Some(owner), Some(pet), t, max) {
            self.cv
                .v
                .h
                .errors
                .push(crate::wiring::action::WiringError::Pet(err));
        }
        None
    }
}

impl<X: Pending + UseRest> UseView<'_, X> {
    /// Owner data `0x0058F030(game, m, owner GUID, owner type, …)`
    /// (`monsters/umod-callbacks.md` §1 rule 5): the minion owner link
    /// of m's AI control record (+0x2C GUID, +0x30 type), looked up by
    /// GUID at every use (`0x0058F0D0`). No owner (GUID −1) drops the
    /// link. A unit without AI control keeps none.
    fn owner_data(&mut self, m: UnitId, owner: Option<UnitId>) {
        let link = owner.and_then(|o| {
            let ty = self.cv.v.units.get(o)?.ty;
            let guid = self.cv.game.lists.unit(o)?.guid;
            Some(crate::monsters::ai::UnitRef { ty, guid })
        });
        if let Some(c) = self.cv.v.h.ai.as_mut().and_then(|s| s.control_mut(m)) {
            c.minion_owner = link;
        }
    }
}

impl<X: Pending> View<'_, X> {
    /// Pets that died or left the game leave their lists (`sim/pets.md`
    /// §6 unlink with kill 0), each with a 0x7A remove to every player
    /// (§8). d2rs-own: the original removes them from the kill's
    /// `0x005751A0`.
    pub fn pet_sweep(&mut self, game: &Game) {
        let owners: Vec<UnitId> = self.h.pet_lists.keys().copied().collect();
        for p in owners {
            let mut gone = Vec::new();
            for e in &mut self.h.pet_lists.get_mut(&p).expect("listed").entries {
                let mut kept = Vec::new();
                for n in std::mem::take(&mut e.nodes) {
                    let dead = game
                        .lists
                        .find_unit(UnitType::Monster, n.guid as u32)
                        .is_none_or(|m| {
                            self.units.get(m).is_none_or(|r| {
                                r.mode == monster_mode::DT || r.mode == monster_mode::DD
                            })
                        });
                    if dead {
                        gone.push(n.guid);
                        e.count -= 1;
                    } else {
                        kept.push(n);
                    }
                }
                e.nodes = kept;
            }
            for g in gone {
                let msg = crate::world::hirelings::pets::pet_action(0, 0, 0, g as u32, 0);
                for c in game.lists.units_of_type(UnitType::Player) {
                    self.h.x.send(c, &msg);
                }
            }
        }
    }
}
