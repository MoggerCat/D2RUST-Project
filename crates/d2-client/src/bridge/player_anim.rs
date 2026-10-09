// Spec: specs/skills/sequences.md (client mode-18 machine, rules 3–5; local player rules 1–4), specs/client/model.md (§8 rule 4), specs/sim/units.md (§4.1, §4.2 frame advance, §4.7), specs/sim/intents-events.md (`0x006217C0`)
//! The client player's animation and its mode end: the player part of
//! the client update `0x00463390` that returns a player to neutral when
//! its attack, cast or hit animation is over.
//!
//! A player mode set (`0x00624690` → `0x00624390`) restarts the frame at
//! the frame bonus (`sim/units.md` §4.1, `0x00623B10`), with the frame
//! count of the mode's AnimData record (frames · 256) and the rate
//! `0x00623F50` (§4.7); the same mode again restarts nothing. Each
//! client update then runs, for a player in an ending mode
//! ([`ENDING_MODES`]): advance the frame unless the animation is
//! complete (`0x006217C0`: frame + speed ≥ count), then, when it is
//! complete, the neutral end (`0x004611F0`: mode 1, 5 in town). The
//! server sends a player's own client nothing when it returns to
//! neutral (`sim/pathing.md` §10 r2: the NU row skips the own client),
//! so without this step the local player stayed in the attack mode and
//! the click gate (`bridge::click::can_act`, modes 7–12 refused) dropped
//! every later click.
//!
//! PROVISIONAL (skills/sequences.md client mode-18 machine; REC-1000):
//! every mode in [`ENDING_MODES`] runs the mode-18 row's steps {advance
//! 1, end 3} on its AnimData frames (the rows of table `0x00711E00` for
//! modes 4 and 7–16 are not written; mode 18's is), so a player ends the
//! mode on the first update whose advanced frame + speed reaches the
//! count; settled by a read of `0x00711E00` rows 4, 7–16 and a recording
//! of the local player's mode per client update across one Multiple Shot
//! and one Frost Nova (`record_state.py` `m`, `fr`, `fc`, `sp`).
//! PROVISIONAL (sim/units.md §4.7; REC-1001): the rate reads no used
//! skill (`seqtrans`, `UseAttackRate`) and no item rate values (stats
//! 93–105 read 0), and a player without a base stat 67–69 reads 100, as
//! the audio feed's `player_rate`; the frame bonus reads no dual wield;
//! settled by the recording of REC-1000 with an IAS / FCR item worn.

use d2_sim::units::anim_rate::{anim_rate, frame_bonus, mode_row, Rate, RateInput};

use super::world::{ClientWorld, ModelInputs, UnitKey, PLAYER};

/// The modes the client update ends in neutral (REC-1000): GH 4, A1 7,
/// A2 8, BL 9, SC 10, TH 11, KK 12, S1–S4 13–16 (the modes the server's
/// event 1 ends in neutral, `sim/units.md` §4.5).
pub const ENDING_MODES: [u32; 11] = [4, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];

/// A player's animation in one mode, as the host's tables give it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PlayerAnim {
    /// AnimData frames of the mode's COF name (`formats/animdata.md` §5).
    pub frames: u32,
    /// AnimData speed (+0x0C).
    pub speed: i32,
    /// The COF weapon class (`render/unit-composite.md` §2.1) as its
    /// `sequences::CLASSES` index (0 `hth`).
    pub weapon: i32,
}

/// The animation lookup of the client player update: the AnimData
/// record of a model player's COF name in `mode`, its weapon class from
/// the model's worn items. `None`: no record (the mode ends on the next
/// update).
pub trait PlayerAnims: std::fmt::Debug + Send + Sync {
    fn anim(&self, w: &ClientWorld, key: UnitKey, mode: u32) -> Option<PlayerAnim>;
}

/// The type class `0x00629FE0` of a COF weapon class index
/// (`render/unit-composite.md` §2.1 r-dual: `bow` 1 … `xbw` 7, `ht1` 12,
/// otherwise 0).
fn type_class(weapon: i32) -> u32 {
    match weapon {
        c @ (1..=7 | 12) => c as u32,
        _ => 0,
    }
}

/// The rate `0x00623F50` of a player in `mode` with AnimData speed `s`
/// (`sim/units.md` §4.7; REC-1001).
fn rate(w: &ClientWorld, key: UnitKey, mode: u32, s: i32) -> i32 {
    let Some(u) = w.units.get(&key) else {
        return s;
    };
    let total = |stat: u16| {
        let base = if u.stats.contains_key(&stat) { 0 } else { 100 };
        base + w.total(key, stat, 0)
    };
    let i = RateInput {
        applies: true,
        t: 0,
        c: u.class,
        m: mode,
        s,
        velocitypercent: total(67),
        attackrate: total(68),
        other_animrate: total(69),
        has_path: true,
        w: if mode == 3 { 101 } else { 213 },
        velocity_mode: mode_row(0, u.class, mode).v,
        player_mode_18: mode == 18,
        ..RateInput::default()
    };
    match anim_rate(&i) {
        Ok(Rate::Set { speed, .. }) => speed,
        _ => s,
    }
}

/// The player mode set (`0x00624690`): a new mode is written and its
/// animation restarted (frame := frame bonus · 256, count := frames ·
/// 256, speed := the rate); the same mode changes nothing. A unit not in
/// the model or not a player: nothing.
pub fn mode_set(w: &mut ClientWorld, inputs: &ModelInputs, key: UnitKey, mode: u32) {
    let Some(u) = w.units.get(&key) else {
        return;
    };
    if key.unit_type != PLAYER || u.mode == mode {
        return;
    }
    let class = u.class;
    let anim = inputs
        .player_anims
        .as_ref()
        .and_then(|a| a.anim(w, key, mode))
        .unwrap_or_default();
    let speed = rate(w, key, mode, anim.speed);
    let bonus = frame_bonus(0, class, mode, false, Some(type_class(anim.weapon)));
    let u = w.units.get_mut(&key).expect("checked above");
    u.mode = mode;
    u.frame = bonus * 256;
    u.frame_count = (anim.frames as i32).wrapping_mul(256);
    u.speed = Some(speed);
}

/// The player part of the client update (`0x00463390`) for a player in
/// an ending mode: advance unless complete, then the neutral end when
/// complete (module docs; REC-1000).
pub fn step(w: &mut ClientWorld, inputs: &ModelInputs, key: UnitKey) {
    let Some(u) = w.units.get_mut(&key) else {
        return;
    };
    if key.unit_type != PLAYER || !ENDING_MODES.contains(&u.mode) {
        return;
    }
    let s = u.speed.unwrap_or(0);
    let complete = |f: i32| f.saturating_add(s) >= u.frame_count;
    if !complete(u.frame) {
        u.frame += s;
    }
    if complete(u.frame) {
        let (neutral, _) = super::modes::neutral_walk(w, key);
        mode_set(w, inputs, key, neutral);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bridge::world::ClientUnit;
    use std::sync::Arc;

    /// Synthetic lookup: every mode 10 frames at speed 256, weapon `hth`.
    #[derive(Debug)]
    struct Ten;
    impl PlayerAnims for Ten {
        fn anim(&self, _: &ClientWorld, _: UnitKey, _: u32) -> Option<PlayerAnim> {
            Some(PlayerAnim {
                frames: 10,
                speed: 256,
                weapon: 0,
            })
        }
    }

    fn world() -> (ClientWorld, UnitKey) {
        let mut w = ClientWorld::default();
        let key = UnitKey::new(PLAYER, 1);
        let mut u = ClientUnit::new(key);
        u.class = 1;
        u.mode = 1;
        w.units.insert(key, u);
        w.local_player = Some(key);
        (w, key)
    }

    // The client mode-18 machine's end on an AnimData mode (REC-1000).
    // Covers: specs/skills/sequences.md §3
    #[test]
    fn a_cast_mode_ends_in_neutral_when_its_animation_completes() {
        let (mut w, key) = world();
        let inputs = ModelInputs {
            player_anims: Some(Arc::new(Ten)),
            ..ModelInputs::default()
        };
        mode_set(&mut w, &inputs, key, 10);
        assert_eq!((w.units[&key].frame, w.units[&key].frame_count), (0, 2560));
        // Updates 1–8 advance to 2048; frame + speed reaches 2560 after
        // update 9's advance (2304 + 256): neutral on update 9.
        for n in 1..=8 {
            step(&mut w, &inputs, key);
            assert_eq!(w.units[&key].mode, 10, "update {n}");
        }
        step(&mut w, &inputs, key);
        assert_eq!(w.units[&key].mode, 1);
        // Neutral is not an ending mode: nothing more.
        step(&mut w, &inputs, key);
        assert_eq!(w.units[&key].mode, 1);
    }

    #[test]
    fn the_same_mode_does_not_restart_and_no_record_ends_at_once() {
        let (mut w, key) = world();
        let inputs = ModelInputs {
            player_anims: Some(Arc::new(Ten)),
            ..ModelInputs::default()
        };
        mode_set(&mut w, &inputs, key, 7);
        step(&mut w, &inputs, key);
        let f = w.units[&key].frame;
        mode_set(&mut w, &inputs, key, 7);
        assert_eq!(w.units[&key].frame, f);
        // Without a lookup the count is 0: complete on the first update.
        let (mut w, key) = world();
        let bare = ModelInputs::default();
        mode_set(&mut w, &bare, key, 7);
        step(&mut w, &bare, key);
        assert_eq!(w.units[&key].mode, 1);
    }
}
