// Spec: specs/client/msg-skills.md (§11 the client do `0x004C6680`, r1–r5), specs/client/model.md (§19 r2 step 2, r3), specs/audio/triggers-2.md (§15 r2)
//! The client skill do: the player update runs it on an action frame
//! of a skill mode (`0x004C68F0` → `0x004C6680`), and its `cltdofunc`
//! (table `0x00727BA8`, 130 entries) makes the client's own effects of
//! the skill: for the novas (function 25, `0x004E34E0`) the 64 client
//! missiles of the ring `0x004C70D0`, whose `TravelSound` the client
//! missile create requests.
//!
//! Not modelled (each named where it would run): the `cltmissile`
//! create of r2 (no skill the recorded checks cast has one), the
//! functions other than 25 (left as they were: no effect, flag 0x40
//! untouched; PROVISIONAL REC-2207), a non-empty `cltcalc1` (a handler
//! error: the client calc evaluator `0x00646CA0` is not in the model),
//! the `dosound` / `tgtsound` requests and the overlay of r3 (zero for
//! every row that reaches here in the checks; the audio driver lists
//! them as pending) and the local player's delay-bar call of r5.

use super::client_missiles::{create, flag, CreateRecord};
use super::dispatch::HandlerError;
use super::msg::skills::level_with_bonuses;
use super::world::{ClientWorld, ModelInputs, UnitKey};

/// Entries of the do table `0x00727BA8` (`[0x00727DB0]`).
pub const TABLE_LEN: i16 = 130;

/// The null entries of the do table: 0 and 97…129.
fn null_entry(f: i16) -> bool {
    f == 0 || (97..TABLE_LEN).contains(&f)
}

/// The class whose ring toggles record flag 0x4000 per missile (§11 r4).
const RING_TOGGLE_CLASS: u32 = 176;

/// `0x004C68F0(U)`: the used skill of U with its level with bonuses
/// (`0x006442A0(U, E, 1)`) → [`client_do`] with w = 0. No used skill:
/// nothing.
pub fn generic_do(
    w: &mut ClientWorld,
    inputs: &ModelInputs,
    key: UnitKey,
) -> Result<(), HandlerError> {
    let Some(e) = w
        .units
        .get(&key)
        .and_then(|u| u.skills.as_ref())
        .and_then(|l| l.current.and_then(|i| l.entries.get(i)))
        .copied()
    else {
        return Ok(());
    };
    let t = &inputs.tables;
    let level = level_with_bonuses(w, key, &t.skills, &t.skilldesc, &e);
    client_do(w, inputs, key, e.skill, level).map(|_| ())
}

/// The client do `0x004C6680(U, skill, level, 0)` (§11 r1–r5). Returns
/// its result (1 = the do ran).
pub fn client_do(
    w: &mut ClientWorld,
    inputs: &ModelInputs,
    key: UnitKey,
    skill: u16,
    level: i32,
) -> Result<i32, HandlerError> {
    // r1.
    let Some(row) = inputs.tables.skills.get(usize::from(skill)).copied() else {
        return Ok(0);
    };
    let f = row.cltdofunc;
    if !(0..TABLE_LEN).contains(&f) {
        return Ok(0);
    }
    // r2: the `cltmissile` create (not modelled, module docs).
    // r3: the function (w = 0: no `ItemTgtDo` replacement).
    let ran = if null_entry(f) {
        set_flag_40(w, key);
        1
    } else {
        match f {
            25 => nova(w, inputs, key, skill, level)?,
            // PROVISIONAL (REC-2207): the other functions are not modelled.
            _ => return Ok(0),
        }
    };
    // r4: the unit's cast light is detached and removed either way.
    super::modes::remove_unit_light(w, key);
    Ok(i32::from(ran != 0))
}

fn set_flag_40(w: &mut ClientWorld, key: UnitKey) {
    if let Some(u) = w.units.get_mut(&key) {
        u.flag_40 = true;
    }
}

/// `cltdofunc` 25 `0x004E34E0(U, skill, level)` (§11 r4): class m :=
/// `0x004F21B0(U, skill)`; m outside the missile table → 0. Else flag
/// 0x40 on U; v := m's `Vel` + `VelLev` · level / 8 (`0x00663270`) +
/// the `cltcalc1` value; the ring `0x004C70D0(U, U, m, −1, skill, level,
/// v)`; 1.
fn nova(
    w: &mut ClientWorld,
    inputs: &ModelInputs,
    key: UnitKey,
    skill: u16,
    level: i32,
) -> Result<i32, HandlerError> {
    let Some(row) = inputs.tables.skills.get(usize::from(skill)).copied() else {
        return Ok(0);
    };
    let m = missile_class(w, key, &row);
    let rows = &inputs.tables.missiles;
    let Some(mrow) = usize::try_from(m).ok().and_then(|i| rows.get(i)) else {
        return Ok(0);
    };
    set_flag_40(w, key);
    let speed = mrow.vel + mrow.vel_lev * level / 8;
    if row.cltcalc1 != u32::MAX {
        return Err(HandlerError::Invalid(
            "client/msg-skills.md §11 r4: cltcalc1 needs the client calc evaluator",
        ));
    }
    ring(w, inputs, key, m as u32, skill, level, speed)?;
    Ok(1)
}

/// `0x004F21B0(U, skill)` (§11 r4): a `progressive` row with a valid
/// `aurastate` and `aurastat1` whose state list on U holds the stat at
/// n > 1 → missile b (n = 2) or c (n ≥ 3); else `cltmissilea`.
fn missile_class(w: &ClientWorld, key: UnitKey, row: &super::world::SkillRow) -> i32 {
    let [a, b, c] = row.cltmissile_abc.map(i32::from);
    if !row.progressive || row.aurastate < 0 || row.aurastat1 < 0 {
        return a;
    }
    let n = u8::try_from(row.aurastate)
        .ok()
        .and_then(|s| w.units.get(&key)?.state_lists.get(&s))
        .and_then(|l| l.get(&(row.aurastat1 as u16, 0)).copied())
        .unwrap_or(0);
    match n {
        2 => b,
        3.. => c,
        _ => a,
    }
}

/// The ring `0x004C70D0(U, U, m, −1, skill, level, v)` (§11 r4): 64
/// client creates at U's position with record flags 3 (| 4 when v ≠ 0),
/// owner U, target offsets i = 0…63 of the ring (`skills/bodies-2.md`
/// §6.7; the client's tables `0x006DACC0` / `0x006DABC0` hold the same
/// values); class 176 toggles flag 0x4000 before each create.
fn ring(
    w: &mut ClientWorld,
    inputs: &ModelInputs,
    key: UnitKey,
    m: u32,
    skill: u16,
    level: i32,
    v: i32,
) -> Result<(), HandlerError> {
    let Some((x, y)) = w.units.get(&key).map(|u| u.cell()) else {
        return Ok(());
    };
    let mut rec = CreateRecord {
        flags: flag::POSITION | flag::TARGET_RELATIVE,
        owner: Some(key),
        class: m,
        x: i32::from(x),
        y: i32::from(y),
        skill: i32::from(skill),
        level,
        ..CreateRecord::default()
    };
    if v != 0 {
        rec.flags |= flag::VELOCITY;
        rec.velocity = v;
    }
    for i in 0..64 {
        (rec.tx, rec.ty) = d2_sim::skills::use_::bodies::ring_offset(i);
        if m == RING_TOGGLE_CLASS {
            rec.flags ^= flag::NO_LIGHT;
        }
        create(w, &inputs.tables.missiles, &rec, inputs.high_light_quality)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bridge::client_missiles::ClientMissileRow;
    use crate::bridge::player_anim::{self, PlayerAnim, PlayerAnims};
    use crate::bridge::skills::{SkillEntry, SkillList, NATIVE};
    use crate::bridge::world::{ClientUnit, SkillRow, MISSILE, PLAYER};
    use std::sync::Arc;

    const FROST_NOVA: u16 = 44;
    const CLASS: u32 = 3;

    fn inputs(cltdofunc: i16) -> ModelInputs {
        let mut i = ModelInputs::default();
        i.tables.skills = vec![SkillRow::default(); 50];
        i.tables.skills[usize::from(FROST_NOVA)] = SkillRow {
            ingame: true,
            cltdofunc,
            cltmissile: -1,
            cltmissile_abc: [CLASS as i16, -1, -1],
            cltcalc1: u32::MAX,
            ..SkillRow::default()
        };
        i.tables.missiles = vec![ClientMissileRow::default(); 4];
        i.tables.missiles[CLASS as usize] = ClientMissileRow {
            vel: 8,
            vel_lev: 4,
            range: 10,
            ..ClientMissileRow::default()
        };
        i
    }

    fn world() -> (ClientWorld, UnitKey) {
        let mut w = ClientWorld::default();
        let key = UnitKey::new(PLAYER, 1);
        let mut u = ClientUnit::new(key);
        u.position = Some((5000, 5000));
        u.skills = Some(SkillList {
            entries: vec![SkillEntry {
                skill: FROST_NOVA,
                base: 3,
                owner: NATIVE,
                ..SkillEntry::default()
            }],
            current: Some(0),
            ..SkillList::default()
        });
        w.units.insert(key, u);
        (w, key)
    }

    fn missiles(w: &ClientWorld) -> Vec<&crate::bridge::client_missiles::ClientMissile> {
        w.objclient.missiles.values().collect()
    }

    // Covers: specs/client/msg-skills.md §11 r1, §11 r4
    #[test]
    fn the_nova_do_makes_the_64_missile_ring_and_sets_flag_40() {
        let (mut w, key) = world();
        assert_eq!(
            client_do(&mut w, &inputs(25), key, FROST_NOVA, 3).unwrap(),
            1
        );
        assert!(w.units[&key].flag_40);
        let ms = missiles(&w);
        assert_eq!(ms.len(), 64);
        assert!(w.objclient.set_c.iter().all(|(k, u)| k.unit_type == MISSILE
            && u.class == CLASS
            && u.position == Some((5000, 5000))));
        // Targets: the caster's position + the ring offsets, in create order.
        let mut targets: Vec<_> = ms.iter().map(|m| m.target_point).collect();
        let mut want: Vec<_> = (0..64)
            .map(|i| {
                let (dx, dy) = d2_sim::skills::use_::bodies::ring_offset(i);
                (5000 + dx, 5000 + dy)
            })
            .collect();
        targets.sort_unstable();
        want.sort_unstable();
        assert_eq!(targets, want);
        // v = 8 + 4 · 3 / 8 = 9 (flag 4, shifted by the create), × 75 / 100.
        assert!(ms.iter().all(|m| m.velocity == (9 << 8) * 75 / 100));
        assert!(ms
            .iter()
            .all(|m| (m.skill, m.level) == (i32::from(FROST_NOVA), 3)));
    }

    // Covers: specs/client/msg-skills.md §11 r2, §11 r3
    #[test]
    fn a_null_entry_only_sets_flag_40_and_bad_ids_do_nothing() {
        let (mut w, key) = world();
        assert_eq!(
            client_do(&mut w, &inputs(0), key, FROST_NOVA, 3).unwrap(),
            1
        );
        assert!(w.units[&key].flag_40 && missiles(&w).is_empty());
        let (mut w, key) = world();
        for f in [-1, 130] {
            assert_eq!(
                client_do(&mut w, &inputs(f), key, FROST_NOVA, 3).unwrap(),
                0
            );
        }
        // A skill outside the table.
        assert_eq!(client_do(&mut w, &inputs(25), key, 50, 3).unwrap(), 0);
        assert!(!w.units[&key].flag_40 && missiles(&w).is_empty());
        // A missile class outside the table: 0, flag untouched.
        let mut i = inputs(25);
        i.tables.skills[usize::from(FROST_NOVA)].cltmissile_abc[0] = 9;
        assert_eq!(client_do(&mut w, &i, key, FROST_NOVA, 3).unwrap(), 0);
        assert!(!w.units[&key].flag_40);
    }

    /// Cast mode 10 with 6 frames at speed 256, event 2 on frame 2.
    #[derive(Debug)]
    struct Cast;
    impl PlayerAnims for Cast {
        fn anim(&self, _: &ClientWorld, _: UnitKey, _: u32) -> Option<PlayerAnim> {
            let mut events = [0; d2_formats::animdata::EVENTS];
            events[2] = 2;
            Some(PlayerAnim {
                frames: 6,
                speed: 256,
                weapon: 0,
                events: Some(events),
            })
        }
    }

    // Covers: specs/client/msg-skills.md §11 r6
    // Covers: specs/client/model.md §19 r2, §19 r3
    #[test]
    fn the_player_update_runs_the_do_once_on_the_update_after_the_event_frame() {
        let (mut w, key) = world();
        w.local_player = Some(key);
        let mut i = inputs(25);
        i.player_anims = Some(Arc::new(Cast));
        player_anim::mode_set(&mut w, &i, key, 10);
        // Update 1 advances 0 → 1, update 2 crosses frame 2 (+0x4E := 2),
        // update 3 runs the do before its advance.
        for n in 1..=2 {
            player_anim::step(&mut w, &i, key).unwrap();
            assert!(missiles(&w).is_empty(), "update {n}");
        }
        assert_eq!(w.units[&key].action_frame, 2);
        player_anim::step(&mut w, &i, key).unwrap();
        assert_eq!(missiles(&w).len(), 64);
        assert!(w.units[&key].flag_40);
        // Flag 0x40 holds the rest of the mode: no second ring; the end
        // clears the used skill and the neutral mode set clears the flag.
        for _ in 0..3 {
            player_anim::step(&mut w, &i, key).unwrap();
        }
        let u = &w.units[&key];
        assert_eq!((u.mode, missiles(&w).len()), (1, 64));
        assert!(!u.flag_40 && u.skills.as_ref().unwrap().current.is_none());
    }

    // Covers: specs/client/model.md §19 r3
    #[test]
    fn a_remote_player_without_an_action_frame_does_the_skill_at_the_mode_end() {
        let (mut w, key) = world();
        let i = ModelInputs {
            player_anims: None,
            ..inputs(25)
        };
        // No record: count 0, complete on the first update; not the
        // local player, flag 0x40 clear → the do at the end.
        player_anim::mode_set(&mut w, &i, key, 10);
        player_anim::step(&mut w, &i, key).unwrap();
        assert_eq!((w.units[&key].mode, missiles(&w).len()), (1, 64));
    }
}
