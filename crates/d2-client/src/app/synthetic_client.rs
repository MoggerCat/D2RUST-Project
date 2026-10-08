// Spec: specs/client/msg-units.md (§1.2 r2), specs/client/msg-skills.md (§2 r3); preview fills: docs/PLAN.md decisions D1–D3
//! (q-smoke-combat) The client tables of `play --synthetic`: the
//! synthetic game has no user files, so `app::play::run` gave the bridge
//! no monster rows and no skill rows. Without them the town NPCs' S→C
//! 0xAC creates no unit (a class without a row is ignored,
//! `msg-units.md` §1.2 r2), so their 0x6D is dropped, and the join's two
//! S→C 0x23 (skill 0 in each hand, `intents-events.md` §8.2 rules 3.7, 7)
//! are rejected (`msg-skills.md` §2 r3: a selected skill must be inside
//! the table). Every headless app test filled these by hand; the play
//! window now gets the same rows.

use bevy::prelude::App;

use super::single_player;
use crate::bridge::world::SkillRow;
use crate::bridge::BridgeResource;

/// d2rs-own, unverified: the synthetic game's client rows on the bridge:
/// [`single_player::synthetic_unit_rows`] and one `skills` row (skill 0,
/// the row every install has).
pub fn install(app: &mut App) {
    let mut b = app.world_mut().resource_mut::<BridgeResource>();
    b.0.set_unit_rows(single_player::synthetic_unit_rows());
    b.0.set_skill_rows(vec![SkillRow::default()]);
}
