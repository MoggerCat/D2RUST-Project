// Spec: specs/missiles/client.md (§C13 function 2), specs/missiles/client-bodies.md (§B3 r1, §B4 function 3, §B5 r2, r4, r5, r6)
//! Client missile function bodies past the first set
//! ([`super::update_with`] dispatches here): 2 (blood), 3 (sub-missile at
//! each new sub-tile, through the missiles evaluator), 59 (height window
//! of the motion record), 65 (`baalfx spirit`), 7 (guided) and 9
//! (meteor centre).

use d2_sim::skills::levels::eval_missile;

use super::super::dispatch::HandlerError;
use super::super::passive::ClientSkills;
use super::super::world::{ClientWorld, UnitKey, MISSILE};
use super::{
    cell_of, create, default_step, end_with, flag, in_town, light_of, remove, repath, repath_to,
    rnd, seed_step, unit_size, ClientMissileRow, CreateRecord, Env,
};

/// Function 2 `0x004D33E0` (`client.md` §C13, blood): at the animation
/// end (frame + speed ≥ length) flag 0x10000 and path velocity 0. Then
/// m's client pixel position (`render/camera.md` §2, a moving unit) less
/// the drawn frame's unit origin getters (§4) in [0, W] × [0, H − 40 +
/// 64] → frames left := 128 and step; else end(none, 0).
pub(super) fn blood(
    w: &mut ClientWorld,
    env: &Env,
    key: UnitKey,
    row: &ClientMissileRow,
) -> Result<(), HandlerError> {
    let origin = w.unit_origin.ok_or(HandlerError::Invalid(
        "missiles/client.md §C13 function 2: blood reads the drawn frame's unit origin",
    ))?;
    let Some(m) = w.objclient.missiles.get_mut(&key) else {
        return Ok(());
    };
    if m.frame + m.anim_speed >= m.anim_len {
        m.flat = true;
        m.velocity = 0;
    }
    let p = crate::rules::camera::moving_to_client(m.pos.0, m.pos.1);
    let (sx, sy) = (p.x - origin.x, p.y - origin.y);
    if (0..=origin.width).contains(&sx) && (0..=origin.play_height + 64).contains(&sy) {
        m.current = 128;
        default_step(w, env, key, row)
    } else {
        end_with(w, env, key, false).map(|_| ())
    }
}

/// Function 3 `0x004D3460` (`client-bodies.md` §B4): the last path step
/// entered a new sub-tile and S1 ≥ 0 → v := the missiles evaluator
/// `0x0064B7C0(m, O, CltCalc1, class, level)` (no formula → 0),
/// sub-at-step(m, S1, 0, v). Then step.
pub(super) fn sub_at_new_step(
    w: &mut ClientWorld,
    env: &Env,
    key: UnitKey,
    row: &ClientMissileRow,
) -> Result<(), HandlerError> {
    let Some(m) = w.objclient.missiles.get(&key).copied() else {
        return Ok(());
    };
    let s1 = row.clt_sub[0];
    if m.new_step && s1 >= 0 {
        let t = env.skills.ok_or(HandlerError::Invalid(
            "missiles/client-bodies.md §B4 function 3: the missiles evaluator needs the skills tables",
        ))?;
        let class = w.objclient.set_c.get(&key).map_or(0, |u| u.class as i32);
        let mut units = ClientSkills::new(w, m.owner.unwrap_or(key));
        let v = eval_missile(
            &mut units,
            t,
            Some(key),
            m.owner,
            row.clt_calc1,
            class,
            m.level,
        );
        sub_at_step(w, env, key, i32::from(s1), 0, v)?;
    }
    default_step(w, env, key, row)
}

/// Sub-at-step `0x004CE050(m, c, range, loops)` (`client-bodies.md` §B3
/// r1): nothing unless the last path step entered a new sub-tile. R:
/// flags 5 (start, velocity 0); range > 0 → 0x8005 with frames := range;
/// loops > 0 → flags |= 8. Owner O (may be none), origin m, class c,
/// start (x, y), m's skill and level, R+0x34 := 2 × level − 2. Create.
pub(super) fn sub_at_step(
    w: &mut ClientWorld,
    env: &Env,
    key: UnitKey,
    c: i32,
    range: i32,
    loops: i32,
) -> Result<Option<UnitKey>, HandlerError> {
    let Some(m) = w.objclient.missiles.get(&key).copied() else {
        return Ok(None);
    };
    if !m.new_step {
        return Ok(None);
    }
    let (x, y) = cell_of(&m);
    let mut rec = CreateRecord {
        flags: flag::POSITION | flag::VELOCITY,
        owner: m.owner,
        origin: Some(key),
        class: c as u32,
        x,
        y,
        skill: m.skill,
        level: m.level,
        loops: 2 * m.level - 2,
        ..CreateRecord::default()
    };
    if range > 0 {
        rec.flags |= flag::RANGE;
        rec.range = range;
    }
    if loops > 0 {
        rec.flags |= flag::LOOPS;
    }
    create(w, env.rows, &rec, env.lights)
}

/// Function 59 `0x004D7FC0` (`client-bodies.md` §B5 r6): z := m's motion
/// z (`0x004DA150`); −d28 ≤ z ≤ d2C → step; else remove.
pub(super) fn height_window(
    w: &mut ClientWorld,
    env: &Env,
    key: UnitKey,
    row: &ClientMissileRow,
) -> Result<(), HandlerError> {
    let Some(m) = w.objclient.missiles.get(&key).copied() else {
        return Ok(());
    };
    let z = m.motion.pos[2];
    if m.d28.wrapping_neg() <= z && z <= m.d2c {
        default_step(w, env, key, row)
    } else {
        remove(w, key);
        Ok(())
    }
}

/// The fixed target and velocity of function 65 (`client-bodies.md`
/// Constants).
const SPIRIT_TARGET: (i32, i32) = (15135, 5900);
const SPIRIT_VELOCITY: i32 = 0xF00;

/// Function 65 `0x004D7E00` (`client-bodies.md` §B5 r4, `baalfx
/// spirit`): b := d2C; U := the set-C missile with GUID d28 (none →
/// step); k := U's d28.
/// 1. b ≠ k and k ≥ 2: rnd(25) = 0 → d2C := k; k = 2 → path velocity
///    0xF00, target point (15135, 5900), re-path; step.
/// 2. b = 0: seed step; lo' mod 10 = 0 → d2C := 1, motion restart, path
///    velocity 0xF00, rnd(2) ≠ 0 → motion velocity (0, 0, 1) ≪; step.
/// 3. b = 1: (dx, dy) := m − U, negated when m's GUID has bit 1 (0x2);
///    (0, 0) → step; else target point := (x − dy, y + dx), re-path; step.
pub(super) fn spirit(
    w: &mut ClientWorld,
    env: &Env,
    key: UnitKey,
    row: &ClientMissileRow,
) -> Result<(), HandlerError> {
    let Some(m) = w.objclient.missiles.get(&key).copied() else {
        return Ok(());
    };
    let b = m.d2c;
    let ukey = UnitKey::new(MISSILE, m.d28 as u32);
    let Some(u) = w.objclient.missiles.get(&ukey).copied() else {
        return default_step(w, env, key, row);
    };
    let k = u.d28;
    // r1.
    if b != k && k >= 2 && rnd(w, key, 25) == 0 {
        if let Some(m) = w.objclient.missiles.get_mut(&key) {
            m.d2c = k;
            if k == 2 {
                m.velocity = SPIRIT_VELOCITY;
                m.target_point = SPIRIT_TARGET;
                m.target_unit = None;
                repath(m);
            }
        }
        return default_step(w, env, key, row);
    }
    match b {
        // r2.
        0 => {
            if seed_step(w, key).is_multiple_of(10) {
                let up = rnd(w, key, 2) != 0;
                if let Some(m) = w.objclient.missiles.get_mut(&key) {
                    m.d2c = 1;
                    m.motion.restart();
                    m.velocity = SPIRIT_VELOCITY;
                    if up {
                        m.motion.vel = [0, 0, 1 << 11];
                    }
                }
            }
        }
        // r3.
        1 => {
            let ((x, y), (ux, uy)) = (cell_of(&m), cell_of(&u));
            let (mut dx, mut dy) = (x - ux, y - uy);
            if key.guid & 2 != 0 {
                (dx, dy) = (-dx, -dy);
            }
            if (dx, dy) != (0, 0) {
                if let Some(m) = w.objclient.missiles.get_mut(&key) {
                    m.target_point = (x - dy, y + dx);
                    m.target_unit = None;
                    repath(m);
                }
            }
        }
        _ => {}
    }
    default_step(w, env, key, row)
}

/// Function 7 `0x004D37F0` (`client-bodies.md` §B5 r2, guided): m's room
/// a town room → remove. d28 bit 0 → a stale target unit (gone from the
/// model) is dropped. T := the path target unit; T dead, or O given and
/// not hostile to T (`0x00465C60`) → none. k := P1 > 0 ? P1 : 5; T,
/// elapsed mod k = 0 and 4 ≤ `0x006416D0(m, T)` ≤ 24 → re-path toward T.
/// Step.
pub(super) fn guided(
    w: &mut ClientWorld,
    env: &Env,
    key: UnitKey,
    row: &ClientMissileRow,
) -> Result<(), HandlerError> {
    if in_town(w, key) {
        remove(w, key);
        return Ok(());
    }
    let Some(m) = w.objclient.missiles.get(&key).copied() else {
        return Ok(());
    };
    if m.d28 & 1 != 0 && m.target_unit.is_some_and(|t| !w.units.contains_key(&t)) {
        if let Some(m) = w.objclient.missiles.get_mut(&key) {
            m.target_unit = None;
        }
    }
    let t = m.target_unit.and_then(|t| {
        let u = w.units.get(&t)?;
        let hostile = m
            .owner
            .is_none_or(|o| super::super::combat::hostile_between(w, env.monsters, o, t));
        (!u.is_dead() && hostile).then_some((t, u.cell()))
    });
    let p1 = row.clt_param[0];
    let k = if p1 > 0 { p1 } else { 5 };
    if let Some((t, at)) = t {
        if (m.total - m.current) % k == 0 {
            let (x, y) = cell_of(&m);
            let (Ok(x), Ok(y)) = (u16::try_from(x), u16::try_from(y)) else {
                return default_step(w, env, key, row);
            };
            let d = super::super::objects::distance_at(
                (x, y),
                unit_size(env, w, key),
                at,
                unit_size(env, w, t),
            );
            if (4..=24).contains(&d) {
                if let Some(m) = w.objclient.missiles.get_mut(&key) {
                    repath_to(m, (i32::from(at.0), i32::from(at.1)));
                }
            }
        }
    }
    default_step(w, env, key, row)
}

/// Function 9 `0x004D39C0` (`client-bodies.md` §B5 r5, meteor centre): S1
/// < 0 or m's skill outside the skills table → remove. a, b, c :=
/// max(P1, 1), max(P2, 1), max(P3, 1); n := the skill evaluator
/// `0x00646CA0(O, calc1 of m's skill, skill, level)` clamped to 1…60, q
/// := 60 / n.
/// 1. (elapsed + 1) mod q = 0, m's light radius r < n → radius r + 1.
/// 2. elapsed = 0: S1 created (flags 1, owner O, start (x, y), m's skill
///    and level), its motion position (−c·a, 0, b·a) ≪ and velocity (c,
///    0, −b) ≪; S2 > 0 the same with S2.
/// 3. elapsed = a − 2: the sound (`audio/triggers-2.md` §16; not
///    modelled: no audio output from the client missiles yet).
///
/// Then step.
pub(super) fn meteor(
    w: &mut ClientWorld,
    env: &Env,
    key: UnitKey,
    row: &ClientMissileRow,
) -> Result<(), HandlerError> {
    let Some(m) = w.objclient.missiles.get(&key).copied() else {
        return Ok(());
    };
    let [s1, s2, _] = row.clt_sub;
    if s1 < 0 {
        remove(w, key);
        return Ok(());
    }
    let t = env.skills.ok_or(HandlerError::Invalid(
        "missiles/client-bodies.md §B5 r5: function 9 needs the skills tables",
    ))?;
    let skill_row = usize::try_from(m.skill).ok().and_then(|s| t.skills.get(s));
    let Some(skill_row) = skill_row else {
        remove(w, key);
        return Ok(());
    };
    let [p1, p2, p3, ..] = row.clt_param;
    let (a, b, c) = (p1.max(1), p2.max(1), p3.max(1));
    let n = {
        let mut units = ClientSkills::new(w, m.owner.unwrap_or(key));
        d2_sim::skills::levels::eval_skill(
            &mut units,
            t,
            m.owner,
            skill_row.calc1,
            m.skill,
            m.level,
        )
    }
    .clamp(1, 60);
    let q = 60 / n;
    let elapsed = m.total - m.current;
    // r1.
    if let Some(id) = light_of(w, key) {
        let r = w.lights.radius(id).unwrap_or(0);
        if (elapsed + 1) % q == 0 && r < n {
            w.lights.set_radius(id, r + 1);
        }
    }
    // r2.
    if elapsed == 0 {
        let (x, y) = cell_of(&m);
        for class in [i32::from(s1)]
            .into_iter()
            .chain((s2 > 0).then_some(i32::from(s2)))
        {
            let rec = CreateRecord {
                flags: flag::POSITION,
                owner: m.owner,
                class: class as u32,
                x,
                y,
                skill: m.skill,
                level: m.level,
                ..CreateRecord::default()
            };
            if let Some(child) = create(w, env.rows, &rec, env.lights)? {
                if let Some(cm) = w.objclient.missiles.get_mut(&child) {
                    // `0x004DA1D0` / `0x004DA200`, both ≪ 11.
                    cm.motion.pos = [(-c * a) << 11, 0, (b * a) << 11];
                    cm.motion.vel = [c << 11, 0, (-b) << 11];
                }
            }
        }
    }
    default_step(w, env, key, row)
}
