// Spec: specs/skills/bodies.md §3.8, §4.2; specs/skills/bodies-2.md §4.6–§4.8; specs/skills/use.md §5
//! The Barbarian's skills in the play preview, headless over the synthetic
//! single-player game: a barbarian (class 4) joins, walks out of town by
//! the cave warp (a monster in a town room is not a legal target of
//! Bash, `bodies.md` §3.8 step 1) and casts through C→S 0x0D / 0x0C.
//!
//! Synthetic fills (no game file, `// d2rs-own, unverified`): the skill
//! rows keep the shapes of the real ones (ids 126 Bash, 130 Howl, 132 Leap,
//! 133 Double Swing, 138 Shout, 149 Battle Orders, 151 Whirlwind) but the
//! numbers are made up; the AnimData record of the barbarian's A1.
//! Provisional parts: REC-152 in `docs/HANDOFF.md` §7.

#[path = "app_barbarian/rig.rs"]
mod rig;

use rig::*;

// Covers: specs/skills/bodies.md §3.8
// Covers: specs/skills/use.md §5.3
#[test]
fn bash_on_a_monster_costs_mana_and_hurts_it() {
    let mut r = Rig::new(&[BASH]);
    r.leave_town();
    let m = r.spawn_monster(1);
    r.select_right(BASH);
    let (mana0, life0) = (r.mana(), r.life(m));
    r.right_click_unit(m);
    r.step(30);
    let errors = r.errors();
    assert!(r.mana() < mana0, "Bash spent mana ({errors})");
    assert!(r.life(m) < life0, "Bash hurt the monster ({errors})");
}
