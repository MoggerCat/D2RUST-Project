// Spec: specs/skills/use.md §1, §7; specs/skills/levels.md §6.4; specs/combat/vitals.md §2
//! [`WiredSkills`]: the [`SkillHost`] of a game whose timer events run
//! the action wiring (`d2_sim::wiring::action::ActionSim`, alone or
//! inside another dispatch: [`ActionEvents`]). Each message builds a
//! [`World`] over the skill use pipeline's `d2-sim` provider
//! (`ActionSim::skill_use`) and the message's staged facts, and runs the
//! `d2-sim` handler its spec names. The skill and vitals tables are the
//! action wiring's (`ActionHooks::tables`, `ActionHooks::vitals`).

use d2_sim::combat::vitals::{self, VitalsTables};
use d2_sim::skills::use_::{self, MsgResult, ServerMsg, TargetError};
use d2_sim::skills::{check_skill_point, spend_skill_point, SkillPointCheck, SkillTables};
use d2_sim::units::UnitId;

use super::super::world::ActionEvents;
use super::world::World;
use super::{code, Call, Handled, LearnRest, SkillHost, SkillRest};
use crate::seams::{ClientId, ResultCode};

/// The skill handlers on the wired sim.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WiredSkills {
    /// Messages the pipeline sent whose layout `server-messages.tsv` does
    /// not give (0x5A, `use.md` OQ9): recorded, not queued.
    pub unsent: Vec<(ClientId, ServerMsg)>,
}

impl<D: ActionEvents> SkillHost<D> for WiredSkills
where
    D::X: SkillRest,
{
    /// 0x3A needs the vitals tables (`ActionHooks::vitals`); without them
    /// it stays a stub.
    fn handle(&mut self, call: Call<'_, D>) -> Option<Handled> {
        let Call {
            game,
            events,
            client,
            msg,
            staged,
        } = call;
        let player = staged.player;
        let action = events.action();
        let tables = action.hooks().tables.clone();
        let vitals_t = action.hooks().vitals.clone();
        if msg.first() == Some(&0x3A) && vitals_t.is_none() {
            return None;
        }
        let (result, sends, point_accept) = action.skill_use(game, |u| {
            let mut w = World::new(u, staged);
            let r = run(&mut w, &tables.skills, vitals_t.as_deref(), player, msg);
            (r, std::mem::take(&mut w.sends), w.point_accept)
        });
        let mut resync = false;
        for m in sends {
            match m {
                ServerMsg::Resync => resync = true,
                other => self.unsent.push((client, other)),
            }
        }
        Some(Handled {
            code: result,
            point_accept,
            resync,
        })
    }
}

/// The handler of `msg[0]` (one of the [`super::Status::Handled`] ids)
/// on `w`. `vitals_t`: the vitals tables (0x3A only).
pub fn run<X: SkillRest>(
    w: &mut World<'_, '_, X>,
    skills: &SkillTables,
    vitals_t: Option<&VitalsTables>,
    u: UnitId,
    msg: &[u8],
) -> ResultCode {
    match msg.first().copied() {
        // `client-messages.tsv`: the 0x0B handler does nothing, returns 0
        // (not a skill message: no `pierce_idx`, §2.4 rule 5).
        Some(0x0B) => ResultCode::Done,
        // The vitals' view is the action wiring's (`VitalsUnits` on its
        // `CombatView`: unit records, stat lists, the refresh `0x0064C040`
        // that queues the unit for its update).
        Some(0x3A) => match vitals_t {
            Some(t) => code(vitals::handle_add_stat_point(&mut w.u.cv, t, u, msg)),
            None => ResultCode::Malformed,
        },
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
pub fn add_skill_point<X: SkillRest>(
    w: &mut World<'_, '_, X>,
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
    LearnRest::after_skill_point(w.x_mut(), u);
    ResultCode::Done
}
