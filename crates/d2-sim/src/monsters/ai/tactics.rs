// Spec: specs/monsters/ai.md §6 (distances), §7 (tactics helpers), §8 (commands)
//! Distances, mode and movement requests with their draws, the velocity
//! request and the AI command list.

use crate::game::Game;
use crate::units::UnitId;

use super::{
    delete_thinks, flag, idle, mode, request_mode, state, AiCommand, AiHost, Ctx, ModeTarget,
    Unhandled, UnitRef,
};

/// `(2·max + min) / 2` of the axis distances, truncated (§6).
pub fn distance_formula(dx: i32, dy: i32) -> i32 {
    let (hi, lo) = if dx >= dy { (dx, dy) } else { (dy, dx) };
    (2 * hi + lo) / 2
}

/// `0x005DC530` no-size distance from (ux, uy) to (x, y).
pub fn distance_no_size(u: (i32, i32), p: (i32, i32)) -> i32 {
    distance_formula(
        u.0.wrapping_sub(p.0).wrapping_abs(),
        u.1.wrapping_sub(p.1).wrapping_abs(),
    )
}

/// `0x005DC380` full-size distance from a to b: each axis distance minus
/// a's size, clamped at 0 (1.14d; D2MOO takes the absolute value).
pub fn distance_full_size(a: (i32, i32), size: i32, b: (i32, i32)) -> i32 {
    let dx = (a.0.wrapping_sub(b.0).wrapping_abs() - size).max(0);
    let dy = (a.1.wrapping_sub(b.1).wrapping_abs() - size).max(0);
    distance_formula(dx, dy)
}

/// Full-size distance between two units.
pub fn unit_distance<W: AiHost + ?Sized>(cx: &Ctx<'_, W>, a: UnitId, b: UnitId) -> i32 {
    distance_full_size(cx.world.position(a), cx.world.size(a), cx.world.position(b))
}

/// The three probe points of "can reach directly" `0x005DC640`
/// (`specs/monsters/ai.md` §6): the target, then the target moved by
/// ±(sy, −sx) where (sx, sy) is the sign of unit − target scaled by k,
/// k from the full-size distance `d` (2 below 3, 3 below 11, 4 below 25,
/// else 3). The call is reachable when any probe's collision line
/// (`0x006229F0`, mask 0x805) is clear.
pub fn reach_directly_probes(unit: (i32, i32), target: (i32, i32), d: i32) -> [(i32, i32); 3] {
    let k = match d {
        i32::MIN..=2 => 2,
        3..=10 => 3,
        11..=24 => 4,
        _ => 3,
    };
    let sx = (unit.0 - target.0).signum() * k;
    let sy = (unit.1 - target.1).signum() * k;
    [
        target,
        (target.0 - sy, target.1 + sx),
        (target.0 + sy, target.1 - sx),
    ]
}

/// `0x005DE190` `AITACTICS_SetVelocity` (§7.3): asserts −126 ≤ speed ≤
/// 126, maps method 1 to 7, and overwrites each nonzero field; steps
/// capped at 77.
pub fn set_velocity<W: AiHost + ?Sized>(
    cx: &mut Ctx<'_, W>,
    unit: UnitId,
    method: i32,
    speed: i32,
    steps: i32,
) {
    if !(-126..=126).contains(&speed) {
        // A fatal assert in 1.14d.
        cx.store
            .unhandled
            .push(Unhandled::VelocityAssert { unit, speed });
        return;
    }
    let method = if method == 1 { 7 } else { method };
    let v = &mut cx.store.entry(unit).velocity;
    if method != 0 {
        v.method = method;
    }
    if speed != 0 {
        v.speed = speed;
    }
    if steps != 0 {
        v.steps = steps.min(77);
    }
}

/// `0x005DDF90` `AITACTICS_ChangeModeAndTargetUnit`.
pub fn mode_at<W: AiHost + ?Sized>(
    game: &mut Game,
    cx: &mut Ctx<'_, W>,
    unit: UnitId,
    m: u8,
    target: Option<UnitId>,
) -> bool {
    let t = match target {
        Some(t) => ModeTarget::Unit(t),
        // T = 0: the builder `0x005A7E60` zeroes the record, so the
        // request carries no unit and the point (0, 0) (`ai.md` §7.1
        // mode request record).
        None => ModeTarget::Point(0, 0),
    };
    request_mode(game, cx, unit, m, t)
}

/// `0x005DEAD0` `AITACTICS_UseSkill` (§7.1): mode < 16: current skill,
/// unit flag 0x40, path step 1, mode change; on failure idle 10. True
/// when the mode change succeeded (the result Diablo's aura and the
/// Ancients test, `ai-bodies-4.md` §7, `ai-bodies-5.md` §12).
pub fn use_skill<W: AiHost + ?Sized>(
    game: &mut Game,
    cx: &mut Ctx<'_, W>,
    unit: UnitId,
    m: u8,
    skill: i32,
    target: ModeTarget,
) -> bool {
    if m >= 16 {
        return false;
    }
    cx.world.set_current_skill(unit, skill);
    cx.world.set_skill_flag(unit);
    cx.world.set_path_steps(unit, 1);
    if !request_mode(game, cx, unit, m, target) {
        idle(game, cx, unit, 10);
        return false;
    }
    true
}

/// `0x005DE000` `AITACTICS_UseSequenceSkill` (§7.1): skill id in range:
/// mode 14, current skill, path step 1, no fallback.
pub fn use_sequence_skill<W: AiHost + ?Sized>(
    game: &mut Game,
    cx: &mut Ctx<'_, W>,
    unit: UnitId,
    skill: i32,
    target: ModeTarget,
) {
    if !cx.world.set_current_skill(unit, skill) {
        return;
    }
    cx.world.set_path_steps(unit, 1);
    request_mode(game, cx, unit, mode::SEQUENCE, target);
}

/// Failure flags of [`move_to`].
pub mod move_flag {
    /// Set control flag 0x40.
    pub const FORCE_LOS: u32 = 1;
    /// Draw: < 70 → wander 4, else idle 10; return 1.
    pub const FALLBACK: u32 = 2;
    /// Delete thinks.
    pub const DELETE_THINKS: u32 = 4;
}

/// `0x005DEB60` `AITACTICS_MoveToTarget` (§7.2): returns 1 on success or
/// a flag-2 fallback, else 0.
pub fn move_to<W: AiHost + ?Sized>(
    game: &mut Game,
    cx: &mut Ctx<'_, W>,
    unit: UnitId,
    target: ModeTarget,
    m: u8,
    steps: i32,
    flags: u32,
) -> bool {
    let mut m = m;
    if m == mode::RUN && cx.world.has_state(unit, state::DECREPIFY) {
        cx.store.entry(unit).velocity = Default::default();
        m = mode::WALK;
    }
    cx.world.set_path_steps(unit, steps);
    if request_mode(game, cx, unit, m, target) {
        return true;
    }
    if flags & move_flag::DELETE_THINKS != 0 {
        delete_thinks(game, unit);
    }
    if flags & move_flag::FORCE_LOS != 0 {
        if let Some(c) = cx.store.control_mut(unit) {
            c.flags |= flag::FORCE_LOS;
        }
    }
    if flags & move_flag::FALLBACK != 0 {
        if cx.chance(unit, 70) {
            wander(game, cx, unit, 4);
        } else {
            idle(game, cx, unit, 10);
        }
        return true;
    }
    false
}

fn unit_target(t: Option<UnitId>) -> ModeTarget {
    match t {
        Some(t) => ModeTarget::Unit(t),
        // Target 0 (CorruptArcher edge case 7): the builder `0x005A7E60`
        // zeroes the record, so the request carries no unit and the point
        // (0, 0) (`ai.md` §7.1 mode request record).
        None => ModeTarget::Point(0, 0),
    }
}

/// `0x005DEC80` walk to the target unit with flags.
pub fn walk_to<W: AiHost + ?Sized>(
    game: &mut Game,
    cx: &mut Ctx<'_, W>,
    unit: UnitId,
    t: Option<UnitId>,
    flags: u32,
) -> bool {
    move_to(game, cx, unit, unit_target(t), mode::WALK, 1, flags)
}

/// `0x005DED00` / `0x005DED20` run to the target unit.
pub fn run_to<W: AiHost + ?Sized>(
    game: &mut Game,
    cx: &mut Ctx<'_, W>,
    unit: UnitId,
    t: Option<UnitId>,
    flags: u32,
) -> bool {
    move_to(game, cx, unit, unit_target(t), mode::RUN, 1, flags)
}

/// `0x005DED40`: velocity method 13 first, then walk to the target.
pub fn walk_to_method13<W: AiHost + ?Sized>(
    game: &mut Game,
    cx: &mut Ctx<'_, W>,
    unit: UnitId,
    t: Option<UnitId>,
    flags: u32,
) -> bool {
    set_velocity(cx, unit, 13, 0, 0);
    walk_to(game, cx, unit, t, flags)
}

/// `0x005DEF80` / `0x005DEFB0` walk / run with n steps (0 → 1).
pub fn move_steps<W: AiHost + ?Sized>(
    game: &mut Game,
    cx: &mut Ctx<'_, W>,
    unit: UnitId,
    t: Option<UnitId>,
    run: bool,
    n: i32,
) -> bool {
    let n = if n == 0 { 1 } else { n };
    let m = if run { mode::RUN } else { mode::WALK };
    move_to(game, cx, unit, unit_target(t), m, n, 0)
}

/// The wander draws of `0x005DE200` (§7.2) around `center`: (a) step,
/// bit 0 = 1 → offsets (n, `roll(n)`), else (`roll(n)`, n); (b) step,
/// bit 0 → negate x; (c) step, bit 0 → negate y.
pub fn wander_point(seed: &mut crate::rng::Seed, center: (i32, i32), n: i32) -> (i32, i32) {
    let (mut dx, mut dy) = if seed.step() & 1 == 1 {
        (n, seed.roll(n) as i32)
    } else {
        let r = seed.roll(n) as i32;
        (r, n)
    };
    if seed.step() & 1 == 1 {
        dx = -dx;
    }
    if seed.step() & 1 == 1 {
        dy = -dy;
    }
    (center.0.wrapping_add(dx), center.1.wrapping_add(dy))
}

/// `0x005DE200` `AITACTICS_WalkCloseToUnit` ("wander n").
pub fn wander<W: AiHost + ?Sized>(
    game: &mut Game,
    cx: &mut Ctx<'_, W>,
    unit: UnitId,
    n: i32,
) -> bool {
    let center = cx.world.position(unit);
    let (x, y) = wander_point(cx.world.seed(unit), center, n);
    move_to(game, cx, unit, ModeTarget::Point(x, y), mode::WALK, 1, 0)
}

/// `0x005DF530` wander near unit t (same draws around t).
pub fn wander_near<W: AiHost + ?Sized>(
    game: &mut Game,
    cx: &mut Ctx<'_, W>,
    unit: UnitId,
    t: UnitId,
    n: i32,
) -> bool {
    let center = cx.world.position(t);
    let (x, y) = wander_point(cx.world.seed(unit), center, n);
    move_to(game, cx, unit, ModeTarget::Point(x, y), mode::WALK, 1, 0)
}

/// `0x005DEFE0` / `0x005DF140` escape from t by n (walk / run): t = 0 →
/// false, nothing done (`ai.md` §7.2); n > 5 → velocity steps n; target
/// own + sign(own − t)·n per axis.
pub fn escape<W: AiHost + ?Sized>(
    game: &mut Game,
    cx: &mut Ctx<'_, W>,
    unit: UnitId,
    t: Option<UnitId>,
    n: i32,
    del: bool,
    run: bool,
) -> bool {
    let Some(t) = t else {
        return false;
    };
    if n > 5 {
        set_velocity(cx, unit, 0, 0, n);
    }
    let own = cx.world.position(unit);
    let from = cx.world.position(t);
    let x = own
        .0
        .wrapping_add((own.0.wrapping_sub(from.0)).signum() * n);
    let y = own
        .1
        .wrapping_add((own.1.wrapping_sub(from.1)).signum() * n);
    let m = if run { mode::RUN } else { mode::WALK };
    let flags = if del { move_flag::DELETE_THINKS } else { 0 };
    move_to(game, cx, unit, ModeTarget::Point(x, y), m, 1, flags)
}

/// `0x005DF7D0` "circle n" (§7.2): one step, low byte < 128 → velocity
/// method 5 else 6 with n steps; then walk toward t.
pub fn circle<W: AiHost + ?Sized>(
    game: &mut Game,
    cx: &mut Ctx<'_, W>,
    unit: UnitId,
    t: Option<UnitId>,
    n: i32,
    del: bool,
) -> bool {
    let low = cx.world.seed(unit).step() & 0xFF;
    let method = if low < 128 { 5 } else { 6 };
    set_velocity(cx, unit, method, 0, n);
    let flags = if del { move_flag::DELETE_THINKS } else { 0 };
    move_to(game, cx, unit, unit_target(t), mode::WALK, 1, flags)
}

// ---- §8 commands ----------------------------------------------------

/// `0x0058EE80`: the current command.
pub fn current_command<W: AiHost + ?Sized>(cx: &Ctx<'_, W>, unit: UnitId) -> Option<AiCommand> {
    let c = cx.store.control(unit)?;
    c.commands.get(c.cur).copied()
}

/// `0x0058ED10`: unlink and free the current command; current := its next
/// (§8). The list is the ring in index order (each command's next is the
/// following index, the last one's next is index 0), so freeing the last
/// index makes index 0 current; freeing the only node empties the ring.
pub fn free_current_command<W: AiHost + ?Sized>(cx: &mut Ctx<'_, W>, unit: UnitId) {
    if let Some(c) = cx.store.control_mut(unit) {
        if c.cur < c.commands.len() {
            c.commands.remove(c.cur);
        }
        if c.cur >= c.commands.len() {
            c.cur = 0;
        }
    }
}

/// `0x0058EF40`: a new command inserted before the current one, becoming
/// current.
pub fn copy_command<W: AiHost + ?Sized>(cx: &mut Ctx<'_, W>, unit: UnitId, cmd: AiCommand) {
    if let Some(c) = cx.store.control_mut(unit) {
        let at = c.cur.min(c.commands.len());
        c.commands.insert(at, cmd);
        c.cur = at;
    }
}

/// `0x0058F0D0`: the minion owner unit.
pub fn minion_owner<W: AiHost + ?Sized>(
    game: &Game,
    cx: &Ctx<'_, W>,
    unit: UnitId,
) -> Option<UnitId> {
    let o: UnitRef = cx.store.control(unit)?.minion_owner?;
    game.lists.find_unit(o.ty, o.guid)
}

/// `0x0058F730`: copy the command to every minion of this unit's minion
/// owner, in minion-list order.
pub fn command_minions<W: AiHost + ?Sized>(
    game: &Game,
    cx: &mut Ctx<'_, W>,
    unit: UnitId,
    cmd: AiCommand,
) {
    let Some(leader) = minion_owner(game, cx, unit) else {
        return;
    };
    let guids = cx
        .store
        .control(leader)
        .map(|c| c.minions.clone())
        .unwrap_or_default();
    for g in guids {
        if let Some(m) = game.lists.find_unit(crate::units::UnitType::Monster, g) {
            copy_command(cx, m, cmd);
        }
    }
}

/// `0x0058EEF0(type, set)` `GetAiCommandFromParam` (§8): no current (the
/// ring is empty) → `None`. Else the index of the first command of type
/// `ty` in the order current's next, its next, …, current (current is
/// tested last; a one-node ring tests only it); `set` makes it current.
pub fn find_command<W: AiHost + ?Sized>(
    cx: &mut Ctx<'_, W>,
    unit: UnitId,
    ty: i32,
    set: bool,
) -> Option<usize> {
    let c = cx.store.control_mut(unit)?;
    let len = c.commands.len();
    if len == 0 {
        return None;
    }
    // A non-empty ring always has a current command (`0x0058ED10` keeps
    // current and last both set or both 0).
    let base = c.cur.min(len - 1);
    let at = (1..=len)
        .map(|i| (base + i) % len)
        .find(|&i| c.commands[i].params[0] == ty)?;
    if set {
        c.cur = at;
    }
    Some(at)
}

/// `0x0058EFA0(type, set)` `SetCurrentAiCommand` (§8): find by type
/// ([`find_command`] without `set`); absent → create (type, 0, 0, 0, 0)
/// through `0x0058EF40` ([`copy_command`]: allocator `0x0058EC90` links
/// it before the current one, and it becomes current); then return
/// `0x0058EEF0(type, set)`. `None` only without an AI control.
pub fn get_or_create_command<W: AiHost + ?Sized>(
    cx: &mut Ctx<'_, W>,
    unit: UnitId,
    ty: i32,
    set: bool,
) -> Option<usize> {
    cx.store.control(unit)?;
    if find_command(cx, unit, ty, false).is_none() {
        copy_command(
            cx,
            unit,
            AiCommand {
                params: [ty, 0, 0, 0, 0],
            },
        );
    }
    find_command(cx, unit, ty, set)
}

/// The command at `index` of the unit's list.
pub fn command_mut<'a, W: AiHost + ?Sized>(
    cx: &'a mut Ctx<'_, W>,
    unit: UnitId,
    index: usize,
) -> Option<&'a mut AiCommand> {
    cx.store.control_mut(unit)?.commands.get_mut(index)
}

/// `0x005DC5C0` `AIUTIL_GetDistanceToCoordinates` (§6): the no-size
/// formula from the unit's path position (the seam's position) to (x, y).
pub fn path_distance<W: AiHost + ?Sized>(cx: &Ctx<'_, W>, unit: UnitId, x: i32, y: i32) -> i32 {
    distance_no_size(cx.world.position(unit), (x, y))
}

/// `0x005DED90`: walk to coordinates, path step count 1 (§7.2 table).
pub fn walk_to_point<W: AiHost + ?Sized>(
    game: &mut Game,
    cx: &mut Ctx<'_, W>,
    unit: UnitId,
    x: i32,
    y: i32,
) -> bool {
    move_to(game, cx, unit, ModeTarget::Point(x, y), mode::WALK, 1, 0)
}

/// "walk in radius of t (a, b)" `0x005DE6D0` → `0x005DE4E0` (§7.2):
/// the host's walk with the unit's staged velocity request (§7.3), which
/// the walk's mode request consumes as [`super::request_mode`] does.
pub fn walk_in_radius<W: AiHost + ?Sized>(
    game: &mut Game,
    cx: &mut Ctx<'_, W>,
    unit: UnitId,
    t: UnitId,
    a: i32,
    b: i32,
) -> bool {
    let mut v = cx.store.get(unit).map(|e| e.velocity).unwrap_or_default();
    let ok = cx.world.walk_in_radius(game, unit, t, a, b, &mut v);
    if let Some(e) = cx.store.units.get_mut(&unit) {
        e.velocity = v;
    }
    ok
}

/// The point `0x005DE4E0` walks to (`specs/monsters/ai.md` §7.2;
/// 1.14d-read, REC-501): d := the full-size distance `0x005DC380` from
/// the unit (of size `size`) at `u` to `t`; s := −1 when d < b, else +1;
/// k := min(|d − b|, a). With ax, ay the axis distances and n :=
/// max(ax + ay, k): kx := ax·k / n and ky := ay·k / n (truncated), then
/// while kx + ky < k both grow by 1. The point is u + sign(t − u)·k_axis·s
/// per axis. There is no early exit: k = 0 or t on u gives u itself.
pub fn radius_point(u: (i32, i32), size: i32, t: (i32, i32), a: i32, b: i32) -> (i32, i32) {
    let d = distance_full_size(u, size, t);
    let s = if d < b { -1 } else { 1 };
    let k = (d - b).abs().min(a);
    let (ax, ay) = ((t.0 - u.0).abs(), (t.1 - u.1).abs());
    let n = (ax + ay).max(k);
    let (mut kx, mut ky) = (0, 0);
    if n > 0 {
        kx = ax * k / n;
        ky = ay * k / n;
        while kx + ky < k {
            kx += 1;
            ky += 1;
        }
    }
    (
        u.0 + (t.0 - u.0).signum() * kx * s,
        u.1 + (t.1 - u.1).signum() * ky * s,
    )
}

/// `0x005DEF30` `WalkToTargetCoordinatesNoSteps` ("walk step 0", §7.2):
/// mode 2 at (x, y), step 0, no flags; the mode-change result.
pub fn walk_step0<W: AiHost + ?Sized>(
    game: &mut Game,
    cx: &mut Ctx<'_, W>,
    unit: UnitId,
    x: i32,
    y: i32,
) -> bool {
    move_to(game, cx, unit, ModeTarget::Point(x, y), mode::WALK, 0, 0)
}

/// `0x005DC480` half-size distance from the unit to (x, y) (§6): each
/// axis distance minus (size / 2 + 1) (unsigned halving), clamped at 0.
pub fn half_size_distance<W: AiHost + ?Sized>(
    cx: &Ctx<'_, W>,
    unit: UnitId,
    x: i32,
    y: i32,
) -> i32 {
    let (ux, uy) = cx.world.position(unit);
    let cut = ((cx.world.size(unit) as u32) / 2) as i32 + 1;
    let dx = (ux.wrapping_sub(x).wrapping_abs() - cut).max(0);
    let dy = (uy.wrapping_sub(y).wrapping_abs() - cut).max(0);
    distance_formula(dx, dy)
}

/// `0x005DEDE0`: run to coordinates (walk with the velocity reset under
/// state 60), path step count 1 (§7.2 table).
pub fn run_to_point<W: AiHost + ?Sized>(
    game: &mut Game,
    cx: &mut Ctx<'_, W>,
    unit: UnitId,
    x: i32,
    y: i32,
) -> bool {
    move_to(game, cx, unit, ModeTarget::Point(x, y), mode::RUN, 1, 0)
}

/// `0x005DF680` `AITACTICS_RunCloseToTargetUnit` ("run near t n", §7.2):
/// the wander draws around t with n as a byte, then a run (a walk with
/// the velocity reset under state 60) there, step 1, no flags.
pub fn run_near<W: AiHost + ?Sized>(
    game: &mut Game,
    cx: &mut Ctx<'_, W>,
    unit: UnitId,
    t: Option<UnitId>,
    n: i32,
) -> bool {
    // `0x005DF680` reads t without a null test (an access violation in
    // 1.14d): t = 0 is unreachable (`ai.md` §2.3 "Target 0 in mode-1 and
    // mode-4 bodies"), asserted, not handled.
    let t = t.expect("run near target 0 (ai.md §2.3)");
    let center = cx.world.position(t);
    let (x, y) = wander_point(cx.world.seed(unit), center, i32::from(n as u8));
    move_to(game, cx, unit, ModeTarget::Point(x, y), mode::RUN, 1, 0)
}
