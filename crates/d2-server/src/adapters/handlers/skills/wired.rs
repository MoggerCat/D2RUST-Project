// Spec: specs/skills/use.md §1, §7; specs/skills/levels.md §6.4; specs/combat/vitals.md §2
//! [`WiredSkills`]: the [`SkillHost`] of a game whose timer events run
//! the action wiring (`d2_sim::wiring::action::ActionSim`, alone or
//! inside another dispatch: [`ActionEvents`]). Each message builds a [`World`]
//! over the game, the wired unit system and the [`SkillSeams`], and runs
//! the `d2-sim` handler its spec names.

use d2_sim::combat::vitals::{self, VitalsTables};
use d2_sim::skills::use_::{self, MsgResult, ServerMsg, TargetError};
use d2_sim::skills::{check_skill_point, spend_skill_point, SkillPointCheck, SkillTables};
use d2_sim::units::UnitId;
use d2_sim::wiring::action::{Pending, View};

use super::super::world::ActionEvents;
use super::seams::SkillSeams;
use super::world::World;
use super::{code, Call, Handled, SkillHost};
use crate::seams::{ClientId, ResultCode};

/// The skill handlers on the wired sim.
pub struct WiredSkills<S> {
    /// `charstats` / `experience` (`vitals.md`).
    pub vitals: VitalsTables,
    pub seams: S,
    /// Messages the pipeline sent whose layout `server-messages.tsv` does
    /// not give (0x5A, `use.md` OQ9): recorded, not queued.
    pub unsent: Vec<(ClientId, ServerMsg)>,
}

impl<S> WiredSkills<S> {
    pub fn new(vitals: VitalsTables, seams: S) -> Self {
        Self {
            vitals,
            seams,
            unsent: Vec::new(),
        }
    }
}

impl<D: ActionEvents, S: SkillSeams> SkillHost<D> for WiredSkills<S> {
    fn handle(&mut self, call: Call<'_, D>) -> Handled {
        let Call {
            game,
            events,
            client,
            msg,
            staged,
        } = call;
        let player = staged.player;
        let sys = &mut events.action().sys;
        let tables = sys.hooks.tables.clone();
        let v = View::of(&mut sys.units, &mut sys.stats, &sys.data, &mut sys.hooks);
        let mut w = World::new(game, v, &mut self.seams, staged);
        let result = run(&mut w, &tables.skills, &self.vitals, player, msg);
        let mut resync = false;
        for m in w.sends.drain(..) {
            match m {
                ServerMsg::Resync => resync = true,
                other => self.unsent.push((client, other)),
            }
        }
        Handled {
            code: result,
            point_accept: w.point_accept,
            resync,
        }
    }

    fn unsent(&self) -> &[(ClientId, ServerMsg)] {
        &self.unsent
    }
}

/// The handler of `msg[0]` (one of [`super::HANDLED`]) on `w`.
pub fn run<X: Pending, S: SkillSeams>(
    w: &mut World<'_, X, S>,
    skills: &SkillTables,
    vitals_t: &VitalsTables,
    u: UnitId,
    msg: &[u8],
) -> ResultCode {
    match msg.first().copied() {
        // `client-messages.tsv`: the 0x0B handler does nothing, returns 0
        // (not a skill message: no `pierce_idx`, §2.4 rule 5).
        Some(0x0B) => ResultCode::Done,
        Some(0x3A) => code(vitals::handle_add_stat_point(w, vitals_t, u, msg)),
        Some(0x3B) => add_skill_point(w, skills, u, msg),
        Some(use_::msg::SELECT_SKILL) => code(use_::select_skill(w, skills, u, msg)),
        _ => match use_::handle_message(w, skills, u, msg) {
            Some(MsgResult::Code(c)) => code(c),
            // `intents-events.md` §2.4 rule 4 gives the two codes
            // `use.md` §1 rule 2 leaves open: bad type → 2, far → 1.
            Some(MsgResult::Unspecified(TargetError::BadType)) => ResultCode::Invalid,
            Some(MsgResult::Unspecified(_)) => ResultCode::Refused,
            None => ResultCode::Malformed,
        },
    }
}

/// 0x3B AddSkillPoint `0x0054BD90` (`levels.md` §6.4): size 3 (checked
/// by the dispatcher), skill = u16 at +1; validator → 2 / 3; spend; then
/// step 5.
pub fn add_skill_point<X: Pending, S: SkillSeams>(
    w: &mut World<'_, X, S>,
    t: &SkillTables,
    u: UnitId,
    m: &[u8],
) -> ResultCode {
    let Some(b) = m.get(1..3) else {
        return ResultCode::Malformed;
    };
    let skill = i32::from(u16::from_le_bytes([b[0], b[1]]));
    match check_skill_point(w, t, u, skill) {
        SkillPointCheck::Code2 => return ResultCode::Invalid,
        SkillPointCheck::Code3 => return ResultCode::Malformed,
        SkillPointCheck::Ok => {}
    }
    spend_skill_point(w, t, u, skill);
    // TODO(levels.md §6.4 step 5): read as running after the spend
    // whether or not a level was added; the result is 0 (OQ5).
    w.s.after_skill_point(u);
    ResultCode::Done
}
