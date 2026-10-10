// Spec: specs/client/model.md (§19 r6, r8.5: the tail and the anim step), specs/client/msg-units.md (§1.2 r6.4–6.5), specs/sim/units.md (§4.7), specs/sim/pathing.md (§8.1 r2)
//! The client monster's animation: the frame (+0x44, 8.8), its count
//! (+0x48) and speed (+0x4C). A mode set (`0x00624690`) restarts the
//! frame at 0 with the mode's AnimData count and the rate `0x00623F50`
//! (`sim/units.md` §4.7, through `d2_sim::units::anim_rate`); the add's
//! set-up draws the first frame from the unit seed (`msg-units.md` §1.2
//! r6.5); the monster update advances it before the unit's messages
//! (`model.md` §19 r8.5, anim kind 0: `0x00623E00`).

use d2_sim::rng::Seed;
use d2_sim::units::anim_rate::{anim_rate, mode_row, Rate, RateInput};

use super::world::{ClientUnit, ClientWorld, ModelInputs, UnitKey};

/// Stats of the rate (`sim/units.md` §4.7).
const STAT_VELOCITYPERCENT: u16 = 67;
const STAT_ATTACKRATE: u16 = 68;
const STAT_OTHER_ANIMRATE: u16 = 69;

/// The speed (+0x4C) of a monster in its current mode (`0x00623F50`):
/// draw type 1, the class's AnimData speed for the mode, the unit's
/// totals of stats 67–69, w = the class's run speed in mode 15 (RN), else
/// its walk speed, and the velocity-mode test of `pathing.md` §8.1 r2
/// (`npc` monsters: modes 2 and 15; others: the mode row's V column).
/// No item values, used skill or dual wield on the client.
/// PROVISIONAL (sim/units.md §4.7; REC-502): the client reads no used
/// skill, so the skill columns (V-skill, A-skill) never apply; settled by
/// a recording of a monster casting in sight.
pub fn rate(w: &ClientWorld, inputs: &ModelInputs, key: UnitKey) -> i32 {
    let Some(u) = w.units.get(&key) else {
        return 0;
    };
    rate_of(u, inputs, |s| w.total(key, s, 0))
}

/// [`rate`] of a unit `u` whose stat totals `total` gives (set C has no
/// state or item lists: its base stats). Without a class row 0; when the
/// rate sets no speed, the unit's own speed.
pub fn rate_of(u: &ClientUnit, inputs: &ModelInputs, total: impl Fn(u16) -> i32) -> i32 {
    let Some(class) = inputs
        .tables
        .monsters
        .get(u.class as usize)
        .and_then(|c| c.as_ref())
    else {
        return 0;
    };
    let m = u.mode;
    let s = class
        .anims
        .get(m as usize)
        .copied()
        .flatten()
        .map_or(0, |(_, s)| s as i32);
    let velocity_mode = if class.npc {
        matches!(m, 2 | 15)
    } else {
        mode_row(1, u.class, m).v
    };
    let i = RateInput {
        applies: true,
        t: 1,
        c: u.class,
        m,
        s,
        velocitypercent: total(STAT_VELOCITYPERCENT),
        attackrate: total(STAT_ATTACKRATE),
        other_animrate: total(STAT_OTHER_ANIMRATE),
        has_path: true,
        w: i32::from(if m == 15 {
            class.run_speed
        } else {
            class.walk_speed
        }),
        velocity_mode,
        ..RateInput::default()
    };
    match anim_rate(&i) {
        Ok(Rate::Set { speed, .. }) => speed,
        _ => u.speed.unwrap_or(0),
    }
}

/// The mode set's animation part (`0x00624690` → `0x00624390`): frame
/// := 0, count := the mode's AnimData frames << 8, speed := [`rate`].
pub fn mode_set(w: &mut ClientWorld, inputs: &ModelInputs, key: UnitKey, mode: u32) {
    let Some(u) = w.units.get_mut(&key) else {
        return;
    };
    u.mode = mode;
    let frames = inputs
        .tables
        .monsters
        .get(u.class as usize)
        .and_then(|c| c.as_ref())
        .and_then(|c| c.anims.get(mode as usize).copied().flatten())
        .map_or(0, |(f, _)| f as i32);
    u.frame = 0;
    u.frame_count = frames << 8;
    let speed = rate(w, inputs, key);
    if let Some(u) = w.units.get_mut(&key) {
        u.speed = Some(speed);
    }
}

/// `msg-units.md` §1.2 r6.5: frame := roll(+0x48) on the unit seed
/// (`0x0045C3E0`, `sim/rng.md` range rule). A unit with no seed in the
/// model keeps frame 0.
pub fn first_frame(w: &mut ClientWorld, key: UnitKey) {
    let Some(u) = w.units.get_mut(&key) else {
        return;
    };
    let Some((lo, hi)) = u.seed else {
        return;
    };
    let mut s = Seed::new(lo, hi);
    u.frame = s.roll(u.frame_count) as i32;
    u.seed = Some((s.lo, s.hi));
}

/// `msg-units.md` §1.2 r6.9: the initial path direction of a monster of
/// set S that is not an `npc`: 0, or the low 6 bits of one step of the
/// unit seed when the class has mode 2 (`modes` bit 2, `0x0046C140`);
/// classes 351 → 0x18, 353 → 0x28, 352, 357, 344 → 0. The base-class
/// rules (96 → 7, 301 → `0x0046C570`) need the `BaseId` link the client
/// tables do not hold: PROVISIONAL (REC-2981), not run.
pub fn first_direction(w: &mut ClientWorld, key: UnitKey, npc: bool, modes: u16) {
    if npc {
        return;
    }
    let Some(u) = w.units.get_mut(&key) else {
        return;
    };
    let mut dir = 0;
    if modes & (1 << 2) != 0 {
        if let Some((lo, hi)) = u.seed {
            let mut s = Seed::new(lo, hi);
            dir = (s.step() & 0x3F) as u8;
            u.seed = Some((s.lo, s.hi));
        }
    }
    dir = match u.class {
        351 => 0x18,
        353 => 0x28,
        352 | 357 | 344 => 0,
        _ => dir,
    };
    u.path_dir = Some(dir);
}

/// The anim step of the monster update (`model.md` §19 r8.5, anim
/// kind 0, `0x00623E00`): frame += speed, wrapping at the count.
/// PROVISIONAL (client/model.md §19 r8.5; REC-503): every mode takes
/// anim kind 0 (the mode-record table's kinds 1–3, the end tests and the
/// mode end are not run: the town's NU and WL are kind 0); settled by a
/// recording of a monster attacking and dying in sight.
pub fn step(w: &mut ClientWorld, key: UnitKey) {
    let Some(u) = w.units.get_mut(&key) else {
        return;
    };
    let speed = u.speed.unwrap_or(0);
    u.frame = u.frame.wrapping_add(speed);
    if u.frame_count > 0 && u.frame >= u.frame_count {
        u.frame = u.frame.rem_euclid(u.frame_count);
    }
}

/// The tail of a pathed mode request (`model.md` §19 r6): r4 ≠ 0 and
/// the total of stat 67 ≠ r4 → base stat 67 += r4 − total, then the rate
/// again.
pub fn velocity_tail(w: &mut ClientWorld, inputs: &ModelInputs, key: UnitKey, r4: i32) {
    if r4 == 0 {
        return;
    }
    let v = w.total(key, STAT_VELOCITYPERCENT, 0);
    if v == r4 {
        return;
    }
    let base = w.base(key, STAT_VELOCITYPERCENT, 0);
    let Some(u) = w.units.get_mut(&key) else {
        return;
    };
    u.stats
        .insert(STAT_VELOCITYPERCENT, base.wrapping_add(r4 - v));
    let speed = rate(w, inputs, key);
    if let Some(u) = w.units.get_mut(&key) {
        u.speed = Some(speed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bridge::world::{ClientUnit, MonsterClass, MONSTER};

    /// Kashya (class 150, `npc`): RCNUHTH 12 frames at 256, RCWLHTH 8 at
    /// 256 (the install's AnimData), walk speed 256; the set-up's stats
    /// 67 = 75, 69 = 100 (`msg-units.md` §1.2 r6.1).
    fn kashya() -> (ClientWorld, ModelInputs, UnitKey) {
        let mut inputs = ModelInputs::default();
        let mut c = MonsterClass {
            npc: true,
            walk_speed: 256,
            run_speed: 128,
            ..MonsterClass::default()
        };
        c.anims[1] = Some((12, 256));
        c.anims[2] = Some((8, 256));
        inputs.tables.monsters = vec![None; 151];
        inputs.tables.monsters[150] = Some(c);
        let key = UnitKey::new(MONSTER, 3);
        let mut u = ClientUnit::new(key);
        u.class = 150;
        u.stats.insert(67, 75);
        u.stats.insert(68, 100);
        u.stats.insert(69, 100);
        let mut w = ClientWorld::default();
        w.units.insert(key, u);
        (w, inputs, key)
    }

    // Covers: specs/sim/units.md §4.7; specs/client/model.md §19
    #[test]
    fn a_walk_restarts_at_0_and_steps_three_quarters_a_tick() {
        // 1.14d under Wine (`-seed 1234`, arrival): Kashya's walk starts
        // at server tick 32 and her drawn WL frame is floor(0.75·(t − 32))
        // mod 8 at every recorded tick 33…89 (frame 6 at tick 73).
        let (mut w, inputs, key) = kashya();
        mode_set(&mut w, &inputs, key, 2);
        assert_eq!(w.units[&key].speed, Some(192));
        assert_eq!(w.units[&key].frame_count, 8 << 8);
        for t in 33..=89 {
            step(&mut w, key);
            let want = (192 * (t - 32) / 256) % 8;
            assert_eq!(w.units[&key].frame >> 8, want, "tick {t}");
        }
    }

    // Covers: specs/sim/units.md §4.7
    #[test]
    fn neutral_steps_at_the_animdata_speed() {
        // Recorded: Kashya's NU frame +1 a tick, wrapping at 12.
        let (mut w, inputs, key) = kashya();
        mode_set(&mut w, &inputs, key, 1);
        assert_eq!(w.units[&key].speed, Some(256));
        for t in 1..=30 {
            step(&mut w, key);
            assert_eq!(w.units[&key].frame >> 8, t % 12);
        }
    }

    // Covers: specs/client/msg-units.md §1.2 r6
    #[test]
    fn the_first_frame_is_a_roll_of_the_count_on_the_unit_seed() {
        let (mut w, inputs, key) = kashya();
        mode_set(&mut w, &inputs, key, 1);
        w.units.get_mut(&key).unwrap().seed = Some((1234, 666));
        first_frame(&mut w, key);
        let mut s = Seed::new(1234, 666);
        let want = s.roll(12 << 8) as i32;
        assert_eq!(w.units[&key].frame, want);
        assert_eq!(w.units[&key].seed, Some((s.lo, s.hi)));
        // No seed in the model: no draw, frame stays.
        w.units.get_mut(&key).unwrap().seed = None;
        first_frame(&mut w, key);
        assert_eq!(w.units[&key].frame, want);
    }

    // Covers: specs/client/model.md §19
    #[test]
    fn the_pathed_tail_takes_stat_67_from_r4() {
        let (mut w, inputs, key) = kashya();
        mode_set(&mut w, &inputs, key, 2);
        velocity_tail(&mut w, &inputs, key, 100);
        assert_eq!(w.units[&key].stats.get(&67), Some(&100));
        assert_eq!(w.units[&key].speed, Some(256));
        // r4 = 0, or equal to the total: nothing.
        velocity_tail(&mut w, &inputs, key, 0);
        velocity_tail(&mut w, &inputs, key, 100);
        assert_eq!(w.units[&key].stats.get(&67), Some(&100));
    }
}
