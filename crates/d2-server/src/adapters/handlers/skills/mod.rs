// Spec: specs/sim/intents-events.md §2.4; specs/skills/use.md §1, §7; specs/skills/levels.md §6.4; specs/combat/vitals.md §2
//! Skill and combat intent handlers: skill use (0x05–0x11), select skill
//! (0x3C), stat points (0x3A), skill points (0x3B).
//!
//! The dispatcher (`crate::dispatch`) has already applied the gate, the
//! exact size and the point / unit parse (§2.2–§2.4) when
//! [`handle`] runs. The handlers are the `d2-sim` functions their specs
//! name, run by the game's world host ([`WorldHost::skill`], a
//! [`SkillHost`] slot); a host without one leaves the ids to the stub.
//! [`wired::WiredSkills`] is the slot of the wired sim
//! (`d2_sim::wiring::action::ActionSim`): it runs the handlers on the
//! skill use pipeline's `d2-sim` provider
//! (`d2_sim::wiring::interaction::UseView`, through [`world::World`]).
//!
//! Server messages: the pipeline's 0x15 (resync) goes to
//! [`SimGame::resyncs`] like the dispatcher's; 0x5A ("can't do that")
//! is recorded in [`wired::WiredSkills::unsent`]. Neither has a layout in
//! `server-messages.tsv` (`use.md` OQ9), so neither is queued. No other
//! S→C message is defined for these handlers.

pub mod wired;
pub mod world;

#[cfg(test)]
mod tests;

use d2_sim::game::Game;
use d2_sim::tick::EventDispatch;
use d2_sim::units::{UnitId, UnitType};
use d2_sim::wiring::action::Pending;
use d2_sim::wiring::interaction::UseRest;

use super::super::SimGame;
use super::world::WorldHost;
use crate::dispatch::{is_point, is_unit};
use crate::seams::{ClientId, Intents, MessageSink, Pos, ResultCode, UnitTarget};

/// Status of a client id in this module.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    /// Routed to a handler here.
    Handled,
    /// No written spec owns the behaviour: stays a stub.
    Stub,
}

/// Every skill / combat client id, its owner spec and status.
pub const IDS: &[(u8, &str, &str, Status)] = &[
    (0x05, "ShiftLeftSkill", "skills/use.md §1", Status::Handled),
    (0x06, "LeftSkillOnUnit", "skills/use.md §1", Status::Handled),
    (
        0x07,
        "ShiftLeftSkillOnUnit",
        "skills/use.md §1",
        Status::Handled,
    ),
    (
        0x08,
        "ShiftLeftSkillHold",
        "skills/use.md §1 rule 6",
        Status::Handled,
    ),
    (
        0x09,
        "LeftSkillOnUnitHold",
        "skills/use.md §1 rule 6",
        Status::Handled,
    ),
    (
        0x0A,
        "ShiftLeftSkillOnUnitHold",
        "skills/use.md §1 rule 6",
        Status::Handled,
    ),
    (
        0x0B,
        "Unused0B",
        "sim/client-messages.tsv (returns 0)",
        Status::Handled,
    ),
    (0x0C, "RightSkill", "skills/use.md §1", Status::Handled),
    (
        0x0D,
        "RightSkillOnUnit",
        "skills/use.md §1",
        Status::Handled,
    ),
    (
        0x0E,
        "ShiftRightSkillOnUnit",
        "skills/use.md §1",
        Status::Handled,
    ),
    (
        0x0F,
        "RightSkillHold",
        "skills/use.md §1 rule 6",
        Status::Handled,
    ),
    (
        0x10,
        "RightSkillOnUnitHold",
        "skills/use.md §1 rule 6",
        Status::Handled,
    ),
    (
        0x11,
        "ShiftRightSkillOnUnitHold",
        "skills/use.md §1 rule 6",
        Status::Handled,
    ),
    (
        0x12,
        "EndInferno",
        "none (client-messages.tsv request only)",
        Status::Stub,
    ),
    (0x3A, "AddStatPoint", "combat/vitals.md §2", Status::Handled),
    (
        0x3B,
        "AddSkillPoint",
        "skills/levels.md §6.4",
        Status::Handled,
    ),
    (
        0x3C,
        "SelectSkill",
        "skills/use.md §7; sim/intents-events.md §2.4 rule 7",
        Status::Handled,
    ),
    (
        0x41,
        "Resurrect",
        "none (client-messages.tsv request only)",
        Status::Stub,
    ),
    (
        0x51,
        "BindHotkey",
        "sim/intents-events.md §2.4 rule 7 (fields only)",
        Status::Stub,
    ),
];

/// The ids routed here.
pub fn handled(id: u8) -> bool {
    IDS.iter()
        .any(|&(i, _, _, s)| i == id && s == Status::Handled)
}

/// What the server stages for one message (`adapters::sim` staging):
/// the inputs of the validators `d2-sim` re-runs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Staged {
    /// The client's player unit.
    pub player: UnitId,
    /// Player data +0x168; `None`: no player data.
    pub last_accept: Option<i32>,
    /// Staged positions of the player and the message's target.
    pub positions: Vec<(UnitId, Pos)>,
    /// The target is an item the player owns.
    pub owned_item: Option<UnitId>,
    /// The target is in another act.
    pub other_act: Option<UnitId>,
}

/// One routed message.
pub struct Call<'a, D> {
    pub game: &'a mut Game,
    pub events: &'a mut D,
    pub client: ClientId,
    /// The drained message (exact size, parsed by the dispatcher).
    pub msg: &'a [u8],
    pub staged: Staged,
}

/// A handler's result and the staged fields it wrote.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Handled {
    pub code: ResultCode,
    /// New player data +0x168.
    pub point_accept: Option<i32>,
    /// The pipeline asked for S→C 0x15.
    pub resync: bool,
}

/// The skill handlers of a game whose events are `D`: a slot of the
/// world host ([`WorldHost::skill`]).
pub trait SkillHost<D> {
    /// The handler of `call.msg[0]`, one of the [`Status::Handled`] ids;
    /// `None`: this slot runs no skill handler, the id stays a stub.
    fn handle(&mut self, call: Call<'_, D>) -> Option<Handled>;
}

/// No skill handlers: every skill id stays a stub.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoSkills;

impl<D> SkillHost<D> for NoSkills {
    fn handle(&mut self, _call: Call<'_, D>) -> Option<Handled> {
        None
    }
}

/// The skill-point calls no written-and-implemented spec provides
/// (`levels.md` §6.4), answered by the action wiring's `Pending` value,
/// the skill list's one owner (`UseRest`).
pub trait LearnRest {
    /// `0x0056C700`.
    fn is_class_skill(&self, u: UnitId, skill: i32) -> bool;
    /// `0x00570080` after the cost check: spend, add a level, refresh,
    /// passive state, `0x00646D60`; refund on failure.
    fn add_skill_level(&mut self, u: UnitId, skill: i32, cost: i32);
    /// §6.4 step 5: `0x0055F4F0(…, 1)`, then `0x0056DE40(unit)`.
    fn after_skill_point(&mut self, u: UnitId);
}

/// What the wired skill handlers need of the action wiring's `Pending`
/// value: its own seams, the skill use pipeline's rest and the
/// skill-point calls.
pub trait SkillRest: Pending + UseRest + LearnRest {}

impl<T: Pending + UseRest + LearnRest> SkillRest for T {}

/// A `d2-sim` handler result as a dispatch result code (§2.3; the
/// handlers return 0–3).
pub fn code(c: i32) -> ResultCode {
    match c {
        0 => ResultCode::Done,
        1 => ResultCode::Refused,
        2 => ResultCode::Invalid,
        _ => ResultCode::Malformed,
    }
}

/// The skill handler of `msg[0]`, if the id is routed here and the
/// world host has a skill slot that runs it; `None` leaves the id to the
/// stub.
pub fn handle<D: EventDispatch, W: WorldHost<D>>(
    sim: &mut SimGame<D, W>,
    client: ClientId,
    msg: &[u8],
    out: &mut dyn MessageSink,
) -> Option<ResultCode> {
    let id = *msg.first()?;
    if !handled(id) {
        return None;
    }
    let player = sim
        .sim_client(client)
        .and_then(|c| sim.game.lists.client(c)?.player)?;
    let staged = staged(sim, client, player, msg);
    let h = sim.world.skill(Call {
        game: &mut sim.game,
        events: &mut sim.events,
        client,
        msg,
        staged,
    })?;
    if let Some(f) = h.point_accept {
        sim.set_point_accept(client, f);
    }
    if h.resync {
        sim.queue_resync(client, out);
    }
    Some(h.code)
}

/// The staged inputs of the message (`intents-events.md` §2.4 rules
/// 3–4).
fn staged<D: EventDispatch, W: WorldHost<D>>(
    sim: &SimGame<D, W>,
    client: ClientId,
    player: UnitId,
    msg: &[u8],
) -> Staged {
    let mut s = Staged {
        player,
        last_accept: sim
            .player_fields(player)
            .and_then(|p| p.data)
            .map(|d| d.last_accept),
        positions: Vec::new(),
        owned_item: None,
        other_act: None,
    };
    let id = msg[0];
    if is_point(id) {
        if let Some(p) = sim.point_state(client) {
            s.positions.push((player, p.player));
        }
    } else if is_unit(id) {
        let word = |o: usize| {
            msg.get(o..o + 4)
                .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        };
        let (Some(ty), Some(guid)) = (word(1), word(5)) else {
            return s;
        };
        let target = UnitType::ALL
            .get(ty as usize)
            .and_then(|&t| sim.game.lists.find_unit(t, guid));
        let Some(target) = target else {
            return s;
        };
        match sim.unit_target(client, ty, guid) {
            UnitTarget::OwnedItem => s.owned_item = Some(target),
            UnitTarget::OtherAct => s.other_act = Some(target),
            UnitTarget::At {
                player: p,
                target: t,
            } => {
                s.positions.push((player, p));
                s.positions.push((target, t));
            }
            UnitTarget::Missing => {}
        }
    }
    s
}
