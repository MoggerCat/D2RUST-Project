// Spec: specs/missiles/client.md (§C13 function 2), specs/missiles/client-bodies.md (§B3 r1, §B4 function 3, §B5 r2, r4, r5, r6)
//! Client missile function bodies past the first set
//! ([`super::update_with`] dispatches here): 2 (blood), 3 (sub-missile at
//! each new sub-tile, through the missiles evaluator), 59 (height window
//! of the motion record), 65 (`baalfx spirit`), 7 (guided) and 9
//! (meteor centre).

use d2_sim::rng::Seed;
use d2_sim::skills::levels::eval_missile;

use super::super::dispatch::HandlerError;
use super::super::passive::ClientSkills;
use super::super::world::{ClientWorld, UnitKey, MISSILE};
use super::{
    cell_of, create_from, default_step, end_with, flag, in_town, light_of, remove, repath,
    repath_to, rnd, seed_step, unit_size, ClientMissileRow, CreateRecord, Env,
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
    create_from(w, env, key, &rec)
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
            if let Some(child) = create_from(w, env, key, &rec)? {
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

/// Function 39 `0x004D6660` (`client.md` §C13): owner given → frame :=
/// 0; step.
pub(super) fn frame_zero(
    w: &mut ClientWorld,
    env: &Env,
    key: UnitKey,
    row: &ClientMissileRow,
) -> Result<(), HandlerError> {
    if let Some(m) = w.objclient.missiles.get_mut(&key) {
        if m.owner.is_some() {
            m.frame = 0;
        }
    }
    default_step(w, env, key, row)
}

/// Disc `0x004CEF50(m, r, c, skill, level, chance, s)` →
/// `0x004CED60` (`client-bodies.md` §B3 r3): owner := m's owner (m is a
/// missile; none → nothing). R: flags 0x201, class c, m's skill and
/// level. For i := −r, −r + s, … ≤ r (y offset, outer), j likewise (x
/// offset, inner), i² + j² ≤ r²: chance > 0 → rnd(chance) ≠ 0 skips the
/// point; start (x + j, y + i); R+0x40 := rnd(c's `RandStart`) (0 when
/// that is ≤ 0); the room containing the start, searched from m's room
/// (no client DRLG: taken as in a room, as the create's r3), none →
/// skipped; else create.
#[allow(clippy::too_many_arguments)]
pub(super) fn disc(
    w: &mut ClientWorld,
    env: &Env,
    key: UnitKey,
    r: i32,
    c: i32,
    chance: i32,
    s: i32,
) -> Result<(), HandlerError> {
    let Some(m) = w.objclient.missiles.get(&key).copied() else {
        return Ok(());
    };
    let Some(owner) = m.owner else {
        return Ok(());
    };
    let (x, y) = cell_of(&m);
    // A class outside the table reads `RandStart` through a null row in
    // 1.14d (no live caller); here it reads 0 and the create makes none.
    let rand_start = usize::try_from(c)
        .ok()
        .and_then(|c| env.rows.get(c))
        .map_or(0, |r| r.rand_start);
    let from = u16::try_from(x)
        .ok()
        .zip(u16::try_from(y).ok())
        .and_then(|(x, y)| w.room_at(x, y));
    let s = s.max(1);
    let mut i = -r;
    while i <= r {
        let mut j = -r;
        while j <= r {
            if i * i + j * j <= r * r && (chance <= 0 || rnd(w, key, chance) == 0) {
                let (px, py) = (x + j, y + i);
                let start_frame = rnd(w, key, rand_start);
                let in_room = w.active_rooms.is_none()
                    || (u16::try_from(px).ok())
                        .zip(u16::try_from(py).ok())
                        .is_some_and(|(px, py)| w.room_from(from.as_ref(), px, py).is_some());
                if in_room {
                    let rec = CreateRecord {
                        flags: flag::POSITION | flag::START_FRAME,
                        owner: Some(owner),
                        class: c as u32,
                        x: px,
                        y: py,
                        skill: m.skill,
                        level: m.level,
                        start_frame,
                        ..CreateRecord::default()
                    };
                    create_from(w, env, key, &rec)?;
                }
            }
            j += s;
        }
        i += s;
    }
    Ok(())
}

/// Function 17 `0x004D44B0` (`client-bodies.md` §B4, curse centre): c :=
/// d04, 0 → S1, < 0 → remove; n := d06, ≤ 0 → remove. elapsed < P1 and
/// elapsed mod 3 = 0 → disc(m, n, c, n > 4 ? ⌊n / 2⌋ : n, n > 4 ? 2 :
/// 1). Step.
pub(super) fn curse_centre(
    w: &mut ClientWorld,
    env: &Env,
    key: UnitKey,
    row: &ClientMissileRow,
) -> Result<(), HandlerError> {
    let Some(m) = w.objclient.missiles.get(&key).copied() else {
        return Ok(());
    };
    let mut c = i32::from(m.d04);
    if c == 0 {
        c = i32::from(row.clt_sub[0]);
    }
    let n = i32::from(m.d06);
    if c < 0 || n <= 0 {
        remove(w, key);
        return Ok(());
    }
    let elapsed = m.total - m.current;
    if elapsed < row.clt_param[0] && elapsed % 3 == 0 {
        let (chance, s) = if n > 4 { (n / 2, 2) } else { (n, 1) };
        disc(w, env, key, n, c, chance, s)?;
    }
    default_step(w, env, key, row)
}

/// Function 18 `0x004D4590` (`client.md` §C13, bone spear trail): S1 < 0
/// → step only. Else when elapsed ≥ `InitSteps` and the last path step
/// entered a new sub-tile: S1 created with flags 1 at m's position (m's
/// owner, skill and level); it takes m's direction, m's precise position
/// (the two path fields `0x006203B0` / `0x00620410`, read as function 6
/// reads them) and motion position (0, 0, m's z). Then step.
pub(super) fn spear_trail(
    w: &mut ClientWorld,
    env: &Env,
    key: UnitKey,
    row: &ClientMissileRow,
) -> Result<(), HandlerError> {
    let Some(m) = w.objclient.missiles.get(&key).copied() else {
        return Ok(());
    };
    let s1 = row.clt_sub[0];
    if s1 >= 0 && m.total - m.current >= i32::from(row.init_steps) && m.new_step {
        let (x, y) = cell_of(&m);
        let rec = CreateRecord {
            flags: flag::POSITION,
            owner: m.owner,
            class: s1 as u32,
            x,
            y,
            skill: m.skill,
            level: m.level,
            ..CreateRecord::default()
        };
        if let Some(child) = create_from(w, env, key, &rec)? {
            if let Some(c) = w.objclient.missiles.get_mut(&child) {
                c.direction = m.direction;
                c.pos = m.pos;
                c.motion.pos = [0, 0, m.motion.pos[2]];
            }
        }
    }
    default_step(w, env, key, row)
}

/// Function 27 `0x004D5090` (`client-bodies.md` §B4, Mephisto's fire
/// wall maker): O none → step. S1 > 0 and the last path step entered a
/// new sub-tile → S1 created with flags 0 (start = origin m's position),
/// owner O, origin m, m's skill and level. elapsed mod max(P1, 1) = 0 →
/// wander. Step.
///
/// Wander: a := d28, b := d2C; one seed step, s := lo' & 3: 0 →
/// (trunc(3(a − b) / 4), trunc(3(a + b) / 4)); 2 → (trunc(3(a + b) / 4),
/// trunc(3(b − a) / 4)); 1, 3 → (a, b). Target point := (x + dx, y + dy)
/// (the target unit dropped), re-path, d28 := dx, d2C := dy.
pub(super) fn wander_maker(
    w: &mut ClientWorld,
    env: &Env,
    key: UnitKey,
    row: &ClientMissileRow,
) -> Result<(), HandlerError> {
    let Some(m) = w.objclient.missiles.get(&key).copied() else {
        return Ok(());
    };
    let Some(owner) = m.owner else {
        return default_step(w, env, key, row);
    };
    let s1 = row.clt_sub[0];
    if s1 > 0 && m.new_step {
        let rec = CreateRecord {
            owner: Some(owner),
            origin: Some(key),
            class: s1 as u32,
            skill: m.skill,
            level: m.level,
            ..CreateRecord::default()
        };
        create_from(w, env, key, &rec)?;
    }
    if (m.total - m.current) % row.clt_param[0].max(1) == 0 {
        let (a, b) = (m.d28, m.d2c);
        let (dx, dy) = match seed_step(w, key) & 3 {
            0 => (3 * (a - b) / 4, 3 * (a + b) / 4),
            2 => (3 * (a + b) / 4, 3 * (b - a) / 4),
            _ => (a, b),
        };
        if let Some(m) = w.objclient.missiles.get_mut(&key) {
            let (x, y) = cell_of(m);
            m.target_point = (x + dx, y + dy);
            m.target_unit = None;
            repath(m);
            (m.d28, m.d2c) = (dx, dy);
        }
    }
    default_step(w, env, key, row)
}

/// Functions 46 `0x004D5710` and 52 `0x004D5F80` (`client-bodies.md`
/// §B4): a pair of S1 children at ±(d28, d2C) on each new sub-tile.
/// 46: S1 < 0 → removed **directly** (`0x00465F00`: no sound stop, no
/// light removal; Edge case 2); elapsed < 2 → d28 := y − ty, d2C := tx −
/// x (the path target point); children with flags 0xB (start (x, y),
/// relative target, loops R+0x34 := P1), owner O. 52: S1 < 0 or O none
/// → remove; children with flags 2 (start = origin m, relative target),
/// owner O, origin m. Both: m's skill and level; offset (d28, d2C), then
/// (−d28, −d2C). Step.
pub(super) fn pair_trail(
    w: &mut ClientWorld,
    env: &Env,
    key: UnitKey,
    row: &ClientMissileRow,
) -> Result<(), HandlerError> {
    let Some(m) = w.objclient.missiles.get(&key).copied() else {
        return Ok(());
    };
    let s1 = row.clt_sub[0];
    let wake = row.clt_do_func == super::FN_WAKE_MAKER;
    if s1 < 0 || (wake && m.owner.is_none()) {
        if wake {
            remove(w, key);
        } else {
            super::super::objects::remove_client_unit(w, key);
        }
        return Ok(());
    }
    let (x, y) = cell_of(&m);
    let (mut d28, mut d2c) = (m.d28, m.d2c);
    if !wake && m.total - m.current < 2 {
        let (tx, ty) = m.target_point;
        (d28, d2c) = (y - ty, tx - x);
        if let Some(m) = w.objclient.missiles.get_mut(&key) {
            (m.d28, m.d2c) = (d28, d2c);
        }
    }
    if m.new_step {
        let mut rec = CreateRecord {
            flags: if wake {
                flag::TARGET_RELATIVE
            } else {
                flag::POSITION | flag::TARGET_RELATIVE | flag::LOOPS
            },
            owner: m.owner,
            class: s1 as u32,
            skill: m.skill,
            level: m.level,
            ..CreateRecord::default()
        };
        if wake {
            rec.origin = Some(key);
        } else {
            (rec.x, rec.y, rec.loops) = (x, y, row.clt_param[0]);
        }
        for (ox, oy) in [(d28, d2c), (-d28, -d2c)] {
            (rec.tx, rec.ty) = (ox, oy);
            create_from(w, env, key, &rec)?;
        }
    }
    default_step(w, env, key, row)
}

/// Function 51 `0x004D5DD0` (`client-bodies-2.md` §B11, recycler
/// delay): S1 ≥ 0 and elapsed = P1 → S1 created with flags 0x2001 at (x,
/// y) (owner O, m's skill and level), then P3 more at (x − P4 + rnd(2P4 +
/// 1), y − P4 + rnd(2P4 + 1)) (x drawn first). S2 ≥ 0, elapsed = P2 and O
/// given → O's flag-ex |= 0x40000 (not drawn) and S2 created with flags
/// 0x2000 (owner O, origin m); its sound (`audio/triggers-2.md` §16) is
/// not modelled. Step.
pub(super) fn recycler(
    w: &mut ClientWorld,
    env: &Env,
    key: UnitKey,
    row: &ClientMissileRow,
) -> Result<(), HandlerError> {
    let Some(m) = w.objclient.missiles.get(&key).copied() else {
        return Ok(());
    };
    let [s1, s2, _] = row.clt_sub;
    let [p1, p2, p3, p4, _] = row.clt_param;
    let elapsed = m.total - m.current;
    let (x, y) = cell_of(&m);
    if s1 >= 0 && elapsed == p1 {
        let mut rec = CreateRecord {
            flags: flag::POSITION | flag::RANDOM_DIRECTION,
            owner: m.owner,
            class: s1 as u32,
            x,
            y,
            skill: m.skill,
            level: m.level,
            ..CreateRecord::default()
        };
        create_from(w, env, key, &rec)?;
        for _ in 0..p3 {
            rec.x = x - p4 + rnd(w, key, 2 * p4 + 1);
            rec.y = y - p4 + rnd(w, key, 2 * p4 + 1);
            create_from(w, env, key, &rec)?;
        }
    }
    if let (true, true, Some(owner)) = (s2 >= 0, elapsed == p2, m.owner) {
        if let Some(o) = w.units.get_mut(&owner) {
            o.flag_ex |= super::FLAG_EX_NOT_DRAWN;
        }
        let rec = CreateRecord {
            flags: flag::RANDOM_DIRECTION,
            owner: Some(owner),
            origin: Some(key),
            class: s2 as u32,
            skill: m.skill,
            level: m.level,
            ..CreateRecord::default()
        };
        create_from(w, env, key, &rec)?;
    }
    default_step(w, env, key, row)
}

/// OY of the frozen orb (`client-bodies-2.md` Constants, `0x006DB5D0`);
/// OX[i] = OY[(i + 16) mod 64].
const ORB_OY: [i32; 64] = [
    0, 2, 5, 8, 11, 14, 16, 19, 21, 23, 24, 26, 27, 28, 29, 29, 30, 29, 29, 28, 27, 26, 24, 23, 21,
    19, 16, 14, 11, 8, 5, 2, 0, -2, -5, -8, -11, -14, -16, -19, -21, -23, -24, -26, -27, -28, -29,
    -29, -30, -29, -29, -28, -27, -26, -24, -23, -21, -19, -16, -14, -11, -8, -5, -2,
];

fn orb_offset(i: usize) -> (i32, i32) {
    (ORB_OY[(i + 16) % 64], ORB_OY[i % 64])
}

/// Function 19 `0x004D46D0` (`client-bodies-2.md` §B11, frozen orb): S1
/// < 0 → remove. elapsed mod max(P1, 1) = 0 → d := |d28 rem 64|; S1
/// created with flags 2 (start = origin m, relative target (OX[d],
/// OY[d])), owner O, origin m; d28 := (P2 + d) rem 64. Step.
pub(super) fn frozen_orb(
    w: &mut ClientWorld,
    env: &Env,
    key: UnitKey,
    row: &ClientMissileRow,
) -> Result<(), HandlerError> {
    let Some(m) = w.objclient.missiles.get(&key).copied() else {
        return Ok(());
    };
    let s1 = row.clt_sub[0];
    if s1 < 0 {
        remove(w, key);
        return Ok(());
    }
    let [p1, p2, ..] = row.clt_param;
    if (m.total - m.current) % p1.max(1) == 0 {
        let d = (m.d28 % 64).unsigned_abs() as usize;
        let (ox, oy) = orb_offset(d);
        if let Some(mm) = w.objclient.missiles.get_mut(&key) {
            mm.d28 = (p2 + d as i32) % 64;
        }
        let rec = CreateRecord {
            flags: flag::TARGET_RELATIVE,
            owner: m.owner,
            origin: Some(key),
            class: s1 as u32,
            tx: ox,
            ty: oy,
            skill: m.skill,
            level: m.level,
            ..CreateRecord::default()
        };
        create_from(w, env, key, &rec)?;
    }
    default_step(w, env, key, row)
}

/// Function 20 `0x004D47F0` (`client-bodies-2.md` §B11, orb nova):
/// elapsed < P1 and elapsed mod max(P2, 1) = 0 → u := trunc((d28 − d2C)
/// / 2), v := trunc((d28 + d2C) / 2); target point := (x + u, y + v),
/// re-path, (d28, d2C) := (u, v). Step.
pub(super) fn orb_nova(
    w: &mut ClientWorld,
    env: &Env,
    key: UnitKey,
    row: &ClientMissileRow,
) -> Result<(), HandlerError> {
    let [p1, p2, ..] = row.clt_param;
    if let Some(m) = w.objclient.missiles.get_mut(&key) {
        let elapsed = m.total - m.current;
        if elapsed < p1 && elapsed % p2.max(1) == 0 {
            let (a, b) = (m.d28, m.d2c);
            let (u, v) = ((a - b) / 2, (a + b) / 2);
            let (x, y) = cell_of(m);
            m.target_point = (x + u, y + v);
            m.target_unit = None;
            repath(m);
            (m.d28, m.d2c) = (u, v);
        }
    }
    default_step(w, env, key, row)
}

/// reseed(v): m's seed := `init_low(v)` = {v, 666} (`client-bodies-2.md`
/// §B8, `0x00650E40`).
fn reseed(w: &mut ClientWorld, key: UnitKey, v: u32) {
    if let Some(u) = w.objclient.set_c.get_mut(&key) {
        let s = Seed::init_low(v);
        u.seed = Some((s.lo, s.hi));
    }
}

/// m's seed low word (`get_lo` `0x00650E50`).
fn seed_lo(w: &ClientWorld, key: UnitKey) -> u32 {
    w.objclient
        .set_c
        .get(&key)
        .and_then(|u| u.seed)
        .map_or(0, |(lo, _)| lo)
}

/// The point test `0x0064CB30(room, x, y, mask)` (`sim/path-placement.md`
/// §2 r2) on the client DRLG: the collision word & mask (0x27 & mask in
/// no room; no client DRLG: 0).
fn point_test(w: &ClientWorld, x: i32, y: i32, mask: u32) -> u32 {
    super::collision_word(w, x, y) & mask
}

/// m's room (`0x00620BB0`): the active room of its sub-tile; with no
/// client DRLG every point counts as in a room (the create's r3).
fn has_room(w: &ClientWorld, m: &super::ClientMissile) -> bool {
    let (x, y) = cell_of(m);
    w.active_rooms.is_none()
        || u16::try_from(x)
            .ok()
            .zip(u16::try_from(y).ok())
            .is_some_and(|(x, y)| w.room_at(x, y).is_some())
}

/// The class pick of functions 13 / 10: c := S1; S1 < S2 → S1 + rnd(S2 −
/// S1 + 1).
fn shard_class(w: &mut ClientWorld, key: UnitKey, row: &ClientMissileRow) -> i32 {
    let [s1, s2, _] = row.clt_sub.map(i32::from);
    if s1 < s2 {
        s1 + rnd(w, key, s2 - s1 + 1)
    } else {
        s1
    }
}

/// Shard `0x004CE320(m, r, k, c)` then fall `0x004CE420(X)`
/// (`client-bodies-2.md` §B10 r3): k ≤ 0, elapsed mod k ≠ 0, O none or
/// m's room none → none. reseed(x + elapsed); start (x + r − 1 −
/// rnd(2(r − 1)), y + r − 1 − rnd(2(r − 1))) (x first); flags 0x2003
/// (relative target (0, 0)), owner O, class c, m's skill and level; the
/// point test (start, mask 4) ≠ 0 → none; else create. Fall: X without a
/// row → removed; v := max(X's P2, 1), h := max(X's P1, v); motion
/// position (0, 0, h) ≪, velocity (0, 0, −v) ≪; total := left := ⌊h /
/// v⌋.
fn shard(
    w: &mut ClientWorld,
    env: &Env,
    key: UnitKey,
    r: i32,
    k: i32,
    c: i32,
) -> Result<(), HandlerError> {
    let Some(m) = w.objclient.missiles.get(&key).copied() else {
        return Ok(());
    };
    let elapsed = m.total - m.current;
    if k <= 0 || elapsed % k != 0 || m.owner.is_none() || !has_room(w, &m) {
        return Ok(());
    }
    let (x, y) = cell_of(&m);
    reseed(w, key, (x + elapsed) as u32);
    let sx = x + r - 1 - rnd(w, key, 2 * (r - 1));
    let sy = y + r - 1 - rnd(w, key, 2 * (r - 1));
    if point_test(w, sx, sy, 4) != 0 {
        return Ok(());
    }
    let rec = CreateRecord {
        flags: flag::POSITION | flag::TARGET_RELATIVE | flag::RANDOM_DIRECTION,
        owner: m.owner,
        class: c as u32,
        x: sx,
        y: sy,
        skill: m.skill,
        level: m.level,
        ..CreateRecord::default()
    };
    let Some(x) = create_from(w, env, key, &rec)? else {
        return Ok(());
    };
    let Some(xrow) = env.rows.get(c as usize) else {
        remove(w, x);
        return Ok(());
    };
    let v = xrow.clt_param[1].max(1);
    let h = xrow.clt_param[0].max(v);
    if let Some(xm) = w.objclient.missiles.get_mut(&x) {
        xm.motion.pos = [0, 0, h << 11];
        xm.motion.vel = [0, 0, (-v) << 11];
        xm.total = h / v;
        xm.current = h / v;
    }
    Ok(())
}

/// The skills row of m's skill (`None`: outside the table) from the
/// skills tables (`None` in [`Env`]: a handler error naming `what`).
fn skill_row<'a>(
    w: &ClientWorld,
    env: &Env<'a>,
    key: UnitKey,
    what: &'static str,
) -> Result<Option<&'a d2_data::tables::Skills>, HandlerError> {
    let t = env.skills.ok_or(HandlerError::Invalid(what))?;
    let skill = w.objclient.missiles.get(&key).map_or(-1, |m| m.skill);
    Ok(usize::try_from(skill).ok().and_then(|s| t.skills.get(s)))
}

/// The skill evaluator `0x00646CA0(O, field, skill, level)` of m
/// (`client-bodies-2.md` §B8 eval); the tables are checked by
/// [`skill_row`] first.
fn eval(w: &ClientWorld, env: &Env, key: UnitKey, field: u32) -> i32 {
    let (Some(t), Some(m)) = (env.skills, w.objclient.missiles.get(&key)) else {
        return 0;
    };
    let mut units = ClientSkills::new(w, m.owner.unwrap_or(key));
    d2_sim::skills::levels::eval_skill(&mut units, t, m.owner, field, m.skill, m.level)
}

/// Function 13 `0x004D3F10` (`client-bodies-2.md` §B11, blizzard
/// centre): S1 < 0 → remove; c := the class pick; no skills row → remove;
/// r := eval(`calc1`), k := eval(`calc2`); k = 0 or r ≤ 0 → remove;
/// shard(m, r, k, c) and fall. Step.
pub(super) fn blizzard(
    w: &mut ClientWorld,
    env: &Env,
    key: UnitKey,
    row: &ClientMissileRow,
) -> Result<(), HandlerError> {
    if row.clt_sub[0] < 0 {
        remove(w, key);
        return Ok(());
    }
    let c = shard_class(w, key, row);
    let Some(skill) = skill_row(
        w,
        env,
        key,
        "missiles/client-bodies-2.md §B11 function 13 needs the skills tables",
    )?
    else {
        remove(w, key);
        return Ok(());
    };
    let (r, k) = (
        eval(w, env, key, skill.calc1),
        eval(w, env, key, skill.calc2),
    );
    if k == 0 || r <= 0 {
        remove(w, key);
        return Ok(());
    }
    shard(w, env, key, r, k, c)?;
    default_step(w, env, key, row)
}

/// Function 10 `0x004D3C40` (`client-bodies-2.md` §B11, monster
/// blizzard): S1 < 0 → remove; c := the class pick; q := trunc(level /
/// max(P3, 1)), r := P1 + max(q, 2), k := max(P2 − q, 3); shard and
/// fall. Step.
pub(super) fn mon_blizzard(
    w: &mut ClientWorld,
    env: &Env,
    key: UnitKey,
    row: &ClientMissileRow,
) -> Result<(), HandlerError> {
    if row.clt_sub[0] < 0 {
        remove(w, key);
        return Ok(());
    }
    let c = shard_class(w, key, row);
    let level = w.objclient.missiles.get(&key).map_or(0, |m| m.level);
    let [p1, p2, p3, ..] = row.clt_param;
    let q = level / p3.max(1);
    let (r, k) = (p1 + q.max(2), (p2 - q).max(3));
    shard(w, env, key, r, k, c)?;
    default_step(w, env, key, row)
}

/// Function 53 `0x004D6080` (`client-bodies-2.md` §B11, tiger fury): S1 <
/// 0 → remove. The last path step entered a new sub-tile → S1 created
/// with flags 0 (start = origin m), owner O, origin m. Then function 7,
/// which steps.
pub(super) fn tiger_fury(
    w: &mut ClientWorld,
    env: &Env,
    key: UnitKey,
    row: &ClientMissileRow,
) -> Result<(), HandlerError> {
    let Some(m) = w.objclient.missiles.get(&key).copied() else {
        return Ok(());
    };
    let s1 = row.clt_sub[0];
    if s1 < 0 {
        remove(w, key);
        return Ok(());
    }
    if m.new_step {
        let rec = CreateRecord {
            owner: m.owner,
            origin: Some(key),
            class: s1 as u32,
            skill: m.skill,
            level: m.level,
            ..CreateRecord::default()
        };
        create_from(w, env, key, &rec)?;
    }
    guided(w, env, key, row)
}

/// Function 48 `0x004D59E0` (`client-bodies-2.md` §B11, eruption
/// centre): S1 < 0, O none, m's room none or m's skill outside the skills
/// table → remove. r := eval(`calc1`) (not clamped), k := max(eval(
/// `calc2`), 1). elapsed mod k = 0 → reseed(x + elapsed); P := (x +
/// rnd(2(r − 1)) − (r − 1), y + rnd(2(r − 1)) − (r − 1)); the point test
/// (P, 0x45) = 0 → S1 created with flags 0x2001 at P (owner O), its dead
/// flag (unit flag 0x10000) set; S2 ≥ 0 → S2 created the same way. Step.
pub(super) fn eruption(
    w: &mut ClientWorld,
    env: &Env,
    key: UnitKey,
    row: &ClientMissileRow,
) -> Result<(), HandlerError> {
    let Some(m) = w.objclient.missiles.get(&key).copied() else {
        return Ok(());
    };
    let [s1, s2, _] = row.clt_sub;
    if s1 < 0 || m.owner.is_none() || !has_room(w, &m) {
        remove(w, key);
        return Ok(());
    }
    let Some(skill) = skill_row(
        w,
        env,
        key,
        "missiles/client-bodies-2.md §B11 function 48 needs the skills tables",
    )?
    else {
        remove(w, key);
        return Ok(());
    };
    let r = eval(w, env, key, skill.calc1);
    let k = eval(w, env, key, skill.calc2).max(1);
    let elapsed = m.total - m.current;
    if elapsed % k == 0 {
        let (x, y) = cell_of(&m);
        reseed(w, key, (x + elapsed) as u32);
        let px = x + rnd(w, key, 2 * (r - 1)) - (r - 1);
        let py = y + rnd(w, key, 2 * (r - 1)) - (r - 1);
        if point_test(w, px, py, 0x45) == 0 {
            let mut rec = CreateRecord {
                flags: flag::POSITION | flag::RANDOM_DIRECTION,
                owner: m.owner,
                class: s1 as u32,
                x: px,
                y: py,
                skill: m.skill,
                level: m.level,
                ..CreateRecord::default()
            };
            if let Some(x) = create_from(w, env, key, &rec)? {
                if let Some(xm) = w.objclient.missiles.get_mut(&x) {
                    xm.flat = true;
                }
            }
            if s2 >= 0 {
                rec.class = s2 as u32;
                create_from(w, env, key, &rec)?;
            }
        }
    }
    default_step(w, env, key, row)
}

/// Function 68 `0x004D5880` (`client-bodies-2.md` §B11, `sucfireball`): S1
/// = 0 → remove (−1 passes). The last path step entered a new sub-tile →
/// S1 created with flags 9 at (x, y) (owner O, loops P1). Step.
pub(super) fn suc_fireball(
    w: &mut ClientWorld,
    env: &Env,
    key: UnitKey,
    row: &ClientMissileRow,
) -> Result<(), HandlerError> {
    let Some(m) = w.objclient.missiles.get(&key).copied() else {
        return Ok(());
    };
    let s1 = row.clt_sub[0];
    if s1 == 0 {
        remove(w, key);
        return Ok(());
    }
    if m.new_step {
        let (x, y) = cell_of(&m);
        let rec = CreateRecord {
            flags: flag::POSITION | flag::LOOPS,
            owner: m.owner,
            class: i32::from(s1) as u32,
            x,
            y,
            skill: m.skill,
            level: m.level,
            loops: row.clt_param[0],
            ..CreateRecord::default()
        };
        create_from(w, env, key, &rec)?;
    }
    default_step(w, env, key, row)
}

/// Function 58 `0x004D63E0` (`client-bodies-2.md` §B11, royal strike
/// chaos ice): elapsed mod max(`Param1`, 1) ≠ 0 → step. reseed(d28); a :=
/// (i16) d2C, b := (i16)(d2C >> 16); seed step: lo' even → (p, q) := (b,
/// −a), odd → (−b, a); dx := trunc((p + 4a) / 4), dy := trunc((q + 4b) /
/// 4), 0 → 1 each; target point := (x + dx, y + dy), re-path; d28 :=
/// `get_lo`; d2C := (dy << 16) + (dx & 0xFFFF). Step.
pub(super) fn chaos_ice(
    w: &mut ClientWorld,
    env: &Env,
    key: UnitKey,
    row: &ClientMissileRow,
) -> Result<(), HandlerError> {
    let Some(m) = w.objclient.missiles.get(&key).copied() else {
        return Ok(());
    };
    if (m.total - m.current) % row.param[0].max(1) != 0 {
        return default_step(w, env, key, row);
    }
    reseed(w, key, m.d28 as u32);
    let a = i32::from(m.d2c as i16);
    let b = i32::from((m.d2c >> 16) as i16);
    let (p, q) = if seed_step(w, key) & 1 == 0 {
        (b, -a)
    } else {
        (-b, a)
    };
    let nz = |v: i32| if v == 0 { 1 } else { v };
    let (dx, dy) = (nz((p + 4 * a) / 4), nz((q + 4 * b) / 4));
    let lo = seed_lo(w, key);
    if let Some(m) = w.objclient.missiles.get_mut(&key) {
        let (x, y) = cell_of(m);
        m.target_point = (x + dx, y + dy);
        m.target_unit = None;
        repath(m);
        m.d28 = lo as i32;
        m.d2c = (dy << 16).wrapping_add(dx & 0xFFFF);
    }
    default_step(w, env, key, row)
}

/// Function 44 `0x004D55C0` (`client-bodies-2.md` §B11, distraction): S1
/// < 0, O none, or O's current skill entry none or of another skill →
/// removed **directly** (`0x00465F00`; Edge case 10). The last path step
/// entered a new sub-tile → spawn(S1). m takes O's position
/// (`0x006505E0`: the new-step flag set when the sub-tile changes). Step.
pub(super) fn distraction(
    w: &mut ClientWorld,
    env: &Env,
    key: UnitKey,
    row: &ClientMissileRow,
) -> Result<(), HandlerError> {
    let Some(m) = w.objclient.missiles.get(&key).copied() else {
        return Ok(());
    };
    let s1 = row.clt_sub[0];
    let owner = m.owner.and_then(|o| w.units.get(&o));
    let using = owner
        .and_then(|o| o.skills.as_ref())
        .and_then(|l| l.current.and_then(|i| l.entries.get(i)))
        .is_some_and(|e| i32::from(e.skill) == m.skill);
    let (true, Some(o), true) = (s1 >= 0, owner, using) else {
        super::super::objects::remove_client_unit(w, key);
        return Ok(());
    };
    let (ox, oy) = o.cell();
    if m.new_step {
        super::spawn(w, env, key, s1 as u32)?;
    }
    if let Some(m) = w.objclient.missiles.get_mut(&key) {
        let old = cell_of(m);
        m.pos = (
            (u32::from(ox) << 16) | 0x8000,
            (u32::from(oy) << 16) | 0x8000,
        );
        m.new_step = cell_of(m) != old;
    }
    if let Some(u) = w.objclient.set_c.get_mut(&key) {
        u.position = Some((ox, oy));
    }
    default_step(w, env, key, row)
}

/// Function 47 `0x004D5950` (`client-bodies-2.md` §B11, molten boulder):
/// `ProgSound` > 0 → the sound test (`audio/triggers-2.md` §16, not
/// modelled) and d28 := the motion record's +0x40 (`0x004DA320`, bounces
/// left). Then function 6, which steps.
pub(super) fn molten_boulder(
    w: &mut ClientWorld,
    env: &Env,
    key: UnitKey,
    row: &ClientMissileRow,
) -> Result<(), HandlerError> {
    if row.prog_sound > 0 {
        if let Some(m) = w.objclient.missiles.get_mut(&key) {
            m.d28 = m.motion.bounces_left;
        }
    }
    super::wall_maker(w, env, key, row)
}
