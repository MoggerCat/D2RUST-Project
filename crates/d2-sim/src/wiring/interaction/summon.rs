// Spec: specs/sim/pets.md §2–§8; specs/skills/bodies.md §6.2
//! Summoned units on the skill pipeline's real providers: the summon
//! spawn's monster creation (`0x005B2F20`, [`BodyWorld::create_monster`])
//! and its pet list add ([`BodyEffect::PetAdd`], `0x00575D90`) run here
//! when [`Pending`] has no provider of its own. The lists are
//! [`ActionHooks::pet_lists`] (player data +0x44), the rules
//! [`crate::player::pets`] over [`PetView`].
//!
//! d2rs-own, unverified (preview): a summon is placed on the aimed point
//! itself (the original's spread search `0x005B2F20` is a path-spec
//! seam); the lists' resync `0x00575900` is a no-op, so a list keeps the
//! maximum its last add set; a pet that dies or vanishes leaves its
//! list on the next [`UseView::pet_sweep`].
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
    // PROVISIONAL (pets.md OQ2): the resync `0x00575900` needs the
    // player's skill formulas; the maximum of the last add stands.
    fn resync(&mut self, _: UnitId) {}
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

    /// [`BodyEffect::PetAdd`] on the lists; every other effect comes back
    /// for [`Pending::body_effect`].
    pub(super) fn pet_effect(
        &mut self,
        e: BodyEffect<UnitId, UnitId, RoomId>,
    ) -> Option<BodyEffect<UnitId, UnitId, RoomId>> {
        if let BodyEffect::SentryLaid {
            m,
            owner,
            skill,
            level,
            shots,
        } = e
        {
            // d2rs-own, unverified (REC-241).
            self.cv.v.h.sentries.insert(
                m,
                crate::wiring::action::Sentry {
                    owner,
                    skill,
                    level,
                    shots,
                    next: 0,
                },
            );
            return None;
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
