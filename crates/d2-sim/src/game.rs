// Spec: specs/sim/tick.md §2, §5; specs/sim/unit-order.md §2.5, §3
//! One game's simulation state as far as Phase 3 has specified it: the
//! frame counter, the unit/room/client lists and the timer queue, plus the
//! operations that touch more than one of them.

use thiserror::Error;

use crate::tick::timer::{CallbackId, TimerError, TimerId, TimerOwner, TimerQueue};
use crate::units::{ListError, RoomId, UnitId, UnitLists, UnitType};

/// Errors of the game-level operations.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum GameError {
    #[error(transparent)]
    List(#[from] ListError),
    #[error(transparent)]
    Timer(#[from] TimerError),
}

/// A game.
#[derive(Clone, Debug, Default)]
pub struct Game {
    /// Frame counter (game +0xA8, D2MOO `dwGameFrame`, §2): signed, 0 at
    /// creation, +1 at the start of every tick.
    pub frame: i32,
    pub lists: UnitLists,
    pub timers: TimerQueue,
}

impl Game {
    pub fn new() -> Self {
        Self::default()
    }

    /// Allocates a GUID (`unit-order.md` §1.3) and adds the unit
    /// (`SUNIT_Add`, §3.1). Seed derivation (`rng.md` §5.3), which comes
    /// first in the original, is the unit allocator's job.
    pub fn spawn_unit(
        &mut self,
        ty: UnitType,
        room: Option<RoomId>,
        allied: bool,
    ) -> Result<UnitId, GameError> {
        let guid = self.lists.guids.alloc(ty);
        Ok(self.lists.add_unit(ty, guid, room, allied)?)
    }

    /// Unit removal `0x00555600` (`unit-order.md` §3.2): list unlinks,
    /// then all the unit's timers are cancelled. Immediate, also during
    /// the timer run.
    pub fn remove_unit(&mut self, unit: UnitId) -> Result<(), GameError> {
        self.lists.remove_unit(unit)?;
        self.timers.remove_unit(unit);
        Ok(())
    }

    /// The timer owner record of a unit.
    pub fn owner(&self, unit: UnitId) -> Result<TimerOwner, GameError> {
        let e = self.lists.unit(unit).ok_or(ListError::UnknownUnit(unit))?;
        Ok(TimerOwner {
            unit,
            unit_type: e.ty,
            guid: e.guid,
        })
    }

    /// Schedules an event for `unit` at the current frame
    /// ([`TimerQueue::schedule`], `tick.md` §5.2–§5.3).
    pub fn schedule_event(
        &mut self,
        unit: UnitId,
        event: u32,
        expire: i32,
        callback: Option<CallbackId>,
        arg1: u32,
        arg2: u32,
    ) -> Result<Option<TimerId>, GameError> {
        let owner = self.owner(unit)?;
        Ok(self
            .timers
            .schedule(self.frame, owner, event, expire, callback, arg1, arg2)?)
    }

    /// `0x005537D0` (`unit-order.md` §2.5): for players or monsters (other
    /// types: nothing), calls `f` for each unit in hash iteration order
    /// that `has_player_body_state` rejects, and stops at the first call
    /// returning true, returning that unit. The next link is read after
    /// the call: `f` must not remove the unit it is given.
    ///
    /// `has_player_body_state` answers "has state 7"; TODO(states spec):
    /// read it from the unit's states once they exist.
    pub fn find_unit_of_type(
        &mut self,
        ty: UnitType,
        mut has_player_body_state: impl FnMut(&Game, UnitId) -> bool,
        mut f: impl FnMut(&mut Game, UnitId) -> bool,
    ) -> Option<UnitId> {
        if !matches!(ty, UnitType::Player | UnitType::Monster) {
            return None;
        }
        let mut cur = self.lists.first_of_type(ty);
        while let Some(u) = cur {
            if !has_player_body_state(self, u) && f(self, u) {
                return Some(u);
            }
            cur = self.lists.next_of_type(u);
        }
        None
    }

    /// `0x005538D0` (`unit-order.md` §2.5): calls `f` for each player
    /// without state 7, in hash iteration order, without an early stop.
    /// Same next-link rule as [`Self::find_unit_of_type`].
    pub fn for_each_player(
        &mut self,
        mut has_player_body_state: impl FnMut(&Game, UnitId) -> bool,
        mut f: impl FnMut(&mut Game, UnitId),
    ) {
        let mut cur = self.lists.first_of_type(UnitType::Player);
        while let Some(u) = cur {
            if !has_player_body_state(self, u) {
                f(self, u);
            }
            cur = self.lists.next_of_type(u);
        }
    }
}
