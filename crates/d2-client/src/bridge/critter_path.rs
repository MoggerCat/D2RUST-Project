// Spec: specs/client/model.md (§5 r6.3–r6.4, §19 r3–r4 codes 0x01 and 0x07, r8), specs/client/msg-units.md (§1.2 r6.2, r6.4, r6.5, r6.9), specs/sim/pathing.md (§3, §8.1, §8.5, §9.4–§9.6)
//! The client path of a C monster (a critter of the room pass, `model.md`
//! §5 r6.3): its dynamic path record, the walk request (code 0x01 of the
//! monster mode machine `0x004AFF60`, §19 r4), and the path half of the
//! monster update `0x004B13A0` (§19 r8: path step `0x004807C0`, the turn
//! `0x00648640`, the mode end of a client-only unit).
//!
//! The path code is `d2-sim`'s (the 1.14d client calls the same
//! `0x00649D00`, `0x00649970`, `0x00650840` as the server) over the
//! client act's active rooms, with the unit footprints of the model on
//! the footprint grids (`client_missiles::stamp_unit_footprints`,
//! PROVISIONAL REC-546). The records live in `objclient.c_paths`, keyed
//! like set C.

use std::collections::BTreeMap;

use d2_sim::drlg::{CollisionGrid, Drlg, TileRect};
use d2_sim::path::footprint::{add_footprint, remove_footprint, FootShape, Footprint, RemoveRule};
use d2_sim::path::record::{alloc_dynamic_path, DynamicKind, DynamicPath, MonsterShape};
use d2_sim::path::tables::PathTables;
use d2_sim::path::walk::find::compute;
use d2_sim::path::walk::velocity::{mode_velocity, set_velocity, STAT_VELOCITYPERCENT};
use d2_sim::path::walk::{PathWorld, Walk, WalkUnits};
use d2_sim::path::CollisionRooms;
use d2_sim::rng::Seed;
use d2_sim::units::{ClientId, RoomId, UnitId, UnitType};

use super::world::{ClientUnit, ClientWorld, ModelInputs, MonsterClass, UnitKey};

/// The C monster in its path context.
const ME: UnitId = UnitId(1);
/// The movement's base: `[0x007A04C4]` is always 0, so 0x400
/// (`render/camera.md` §5; `0x00650840`).
const BASE: i32 = 0x400;
/// Unit flag-ex 0x2000, the walk flag W(f) (§19 r4).
const WALK_FLAG: u32 = 0x2000;

/// The client act's active rooms over the footprint grids.
struct Rooms<'a> {
    drlg: &'a Drlg,
    grids: &'a mut BTreeMap<RoomId, CollisionGrid>,
}

impl CollisionRooms for Rooms<'_> {
    fn subtile_rect(&self, room: RoomId) -> Option<TileRect> {
        let r = self.drlg.drlg_room_of(room)?;
        self.drlg.active_room(r).map(|a| a.subtiles)
    }
    fn adjacent_count(&self, room: RoomId) -> usize {
        self.drlg
            .drlg_room_of(room)
            .and_then(|r| self.drlg.active_room(r))
            .map_or(0, |a| a.adjacency.len())
    }
    fn adjacent(&self, room: RoomId, i: usize) -> Option<RoomId> {
        let r = self.drlg.drlg_room_of(room)?;
        let n = *self.drlg.active_room(r)?.adjacency.get(i)?;
        self.drlg.active_room(n).map(|a| a.id)
    }
    fn grid(&self, room: RoomId) -> Option<&CollisionGrid> {
        if let Some(g) = self.grids.get(&room) {
            return Some(g);
        }
        let r = self.drlg.drlg_room_of(room)?;
        self.drlg.active_room(r).map(|a| &a.collision)
    }
    fn grid_mut(&mut self, room: RoomId) -> Option<&mut CollisionGrid> {
        if !self.grids.contains_key(&room) {
            let r = self.drlg.drlg_room_of(room)?;
            let g = self.drlg.active_room(r)?.collision.clone();
            self.grids.insert(room, g);
        }
        self.grids.get_mut(&room)
    }
}

/// The path context of one C monster: its rooms, its footprint as it
/// stands, and what the velocity and compute read of it.
struct Ctx<'a> {
    rooms: Rooms<'a>,
    foot: Footprint,
    class: u32,
    mode: u32,
    percent: i32,
    velocity: i32,
    npc: bool,
    in_town: bool,
    seed: Seed,
}

impl CollisionRooms for Ctx<'_> {
    fn subtile_rect(&self, room: RoomId) -> Option<TileRect> {
        self.rooms.subtile_rect(room)
    }
    fn adjacent_count(&self, room: RoomId) -> usize {
        self.rooms.adjacent_count(room)
    }
    fn adjacent(&self, room: RoomId, i: usize) -> Option<RoomId> {
        self.rooms.adjacent(room, i)
    }
    fn grid(&self, room: RoomId) -> Option<&CollisionGrid> {
        self.rooms.grid(room)
    }
    fn grid_mut(&mut self, room: RoomId) -> Option<&mut CollisionGrid> {
        self.rooms.grid_mut(room)
    }
}

impl PathWorld for Ctx<'_> {
    fn load_path(&self, _: UnitId) -> Option<DynamicPath> {
        None
    }
    fn store_path(&mut self, _: UnitId, _: &DynamicPath) {}
    fn room_in_town(&self, room: RoomId) -> bool {
        let Some(r) = self.rooms.drlg.drlg_room_of(room) else {
            return false;
        };
        let d = self.rooms.drlg;
        d2_sim::drlg::is_town(d.level(d.room(r).level).id)
    }
    fn remove_footprint(&mut self, unit: UnitId, force: bool) -> bool {
        if unit != ME {
            return false;
        }
        let fp = self.foot.clone();
        remove_footprint(&mut self.rooms, &fp, RemoveRule::Other, force)
    }
    fn add_footprint(&mut self, unit: UnitId) {
        if unit == ME {
            let fp = self.foot.clone();
            add_footprint(&mut self.rooms, &fp);
        }
    }
    fn room_list_remove(&mut self, _: UnitId, _: RoomId) {}
    fn room_list_insert(&mut self, _: UnitId, _: RoomId) {}
    fn queue_for_update(&mut self, _: UnitId) {}
    fn room_clients(&self, _: RoomId) -> Vec<ClientId> {
        Vec::new()
    }
}

impl WalkUnits for Ctx<'_> {
    fn unit_type(&self, _: UnitId) -> UnitType {
        UnitType::Monster
    }
    fn class(&self, _: UnitId) -> u32 {
        self.class
    }
    fn frame(&self) -> i32 {
        0
    }
    fn mode(&self, _: UnitId) -> u32 {
        self.mode
    }
    fn stat(&self, _: UnitId, stat: u16) -> i32 {
        if stat == STAT_VELOCITYPERCENT {
            self.percent
        } else {
            0
        }
    }
    fn seed(&mut self, _: UnitId) -> &mut Seed {
        &mut self.seed
    }
    fn monstats_velocity(&self, _: UnitId) -> (i32, bool) {
        (self.velocity, self.npc)
    }
    fn monster_can_be_in_town(&self, _: UnitId) -> bool {
        self.npc || self.in_town
    }
}

fn footprint_of(p: &DynamicPath) -> Footprint {
    Footprint {
        room: p.room,
        x: p.x(),
        y: p.y(),
        shape: FootShape::Pattern(p.pattern),
        mask: p.foot_mask,
    }
}

fn row(inputs: &ModelInputs, class: u32) -> Option<MonsterClass> {
    inputs.tables.monsters.get(class as usize).copied().flatten()
}

/// Runs `f` on the path of `key` in its context. `None`: no client DRLG,
/// no path, no unit or no class row.
fn with_ctx<R>(
    w: &mut ClientWorld,
    inputs: &ModelInputs,
    key: UnitKey,
    f: impl FnOnce(&PathTables, &mut Ctx<'_>, &mut DynamicPath) -> R,
) -> Option<R> {
    let t = super::predict::path_tables()?;
    let u = w.objclient.set_c.get(&key)?;
    let r = row(inputs, u.class)?;
    let (class, mode) = (u.class, u.mode);
    let percent = u.stats.get(&STAT_VELOCITYPERCENT).copied().unwrap_or(0);
    let mut path = w.objclient.c_paths.remove(&key)?;
    let Some(d) = w.drlg.as_ref() else {
        w.objclient.c_paths.insert(key, path);
        return None;
    };
    let mut c = Ctx {
        rooms: Rooms {
            drlg: &d.drlg,
            grids: &mut w.objclient.unit_grids,
        },
        foot: footprint_of(&path),
        class,
        mode,
        percent,
        velocity: r.setup.map_or(0, |s| i32::from(s.velocity)),
        npc: r.npc,
        in_town: r.in_town,
        seed: Seed::default(),
    };
    let out = f(t, &mut c, &mut path);
    w.objclient.c_paths.insert(key, path);
    Some(out)
}

/// The set-up `0x004AE8D0` of a critter made by the room pass
/// (`msg-units.md` §1.2 r6, through the creator `0x00466360`, `model.md`
/// §5 r6.3): the table stats (r6.1), the path at (x, y) in `room` with
/// velocity := `monstats` `Velocity` << 8 (r6.2; its footprint is
/// stamped), mode 1 NU (r6.4: frame 0, the count and the rate), the
/// first frame from the unit seed (r6.5), the flags (r6.6), the initial
/// direction (r6.9) and the creator's stop distance (`monstats2` +0x0E,
/// `0x00649070`).
pub fn setup(w: &mut ClientWorld, inputs: &ModelInputs, key: UnitKey, room: RoomId, x: i32, y: i32) {
    let Some(r) = w
        .objclient
        .set_c
        .get(&key)
        .and_then(|u| row(inputs, u.class))
    else {
        return;
    };
    let Some(t) = super::predict::path_tables() else {
        return;
    };
    let (difficulty, expansion) = (w.difficulty, w.expansion != 0);
    if let (Some(u), Some(s)) = (w.objclient.set_c.get_mut(&key), &r.setup) {
        super::msg::units::setup_stats(u, s, difficulty, expansion);
    }
    let class = w.objclient.set_c.get(&key).map_or(0, |u| u.class);
    let Some(d) = w.drlg.as_ref() else {
        return;
    };
    let mut rooms = Rooms {
        drlg: &d.drlg,
        grids: &mut w.objclient.unit_grids,
    };
    let shape = MonsterShape {
        size_x: i32::from(r.size_x),
        base_id: class,
        npc: r.npc,
        in_town: r.in_town,
        interact: r.interact,
        ..MonsterShape::default()
    };
    let Ok(mut path) = alloc_dynamic_path(
        t,
        &mut rooms,
        DynamicKind::Monster(shape),
        ME,
        Some(room),
        x,
        y,
        false,
    ) else {
        return;
    };
    set_velocity(&mut path, r.setup.map_or(0, |s| i32::from(s.velocity)) << 8);
    w.objclient.c_paths.insert(key, path);
    mode_set(w, inputs, key, 1, true);
    let Some(u) = w.objclient.set_c.get_mut(&key) else {
        return;
    };
    // r6.5: the first frame.
    if let Some((lo, hi)) = u.seed {
        let mut s = Seed::new(lo, hi);
        u.frame = s.roll(u.frame_count) as i32;
        u.seed = Some((s.lo, s.hi));
    }
    if let Some(s) = &r.setup {
        super::msg::units::setup_flags(u, s);
    }
    // r6.9: no direction draw for an `npc`; else one step of the unit
    // seed when the class has mode 2 WL (`0x0046C140`), low 6 bits.
    // The base-class and class overrides of r6.9 name no critter.
    if r.npc {
        return;
    }
    let mut dir = 0;
    if r.modes & (1 << 2) != 0 {
        if let Some((lo, hi)) = u.seed {
            let mut s = Seed::new(lo, hi);
            dir = (s.step() & 0x3F) as u8;
            u.seed = Some((s.lo, s.hi));
        }
    }
    let stop = r.path_byte;
    if let Some(p) = w.objclient.c_paths.get_mut(&key) {
        snap(p, dir);
        // `0x00649070(path, n)`: n − 1 for n in 1…19, else 0.
        p.stop_distance = if (1..=19).contains(&stop) { stop - 1 } else { 0 };
    }
}

/// `0x006488A0(path, d)`: direction := new direction := d & 63.
fn snap(p: &mut DynamicPath, d: u8) {
    p.direction = d & 63;
    p.new_direction = d & 63;
}

/// The mode set of a C monster (`0x00480E70` → `0x00624690`; §19 r1): the
/// same mode restarts nothing unless `restart` (`0x00624390`); a new one
/// sets frame 0, the mode's count and the rate `0x00623F50` with its
/// velocity half (`sim/pathing.md` §8.1: a mode without the modifier
/// keeps the path's velocity).
fn mode_set(w: &mut ClientWorld, inputs: &ModelInputs, key: UnitKey, mode: u32, restart: bool) {
    let Some(u) = w.objclient.set_c.get_mut(&key) else {
        return;
    };
    if u.mode == mode && !restart {
        return;
    }
    u.mode = mode;
    let r = row(inputs, u.class);
    let frames = r
        .and_then(|c| c.anims.get(mode as usize).copied().flatten())
        .map_or(0, |(f, _)| f as i32);
    u.frame = 0;
    u.frame_count = frames << 8;
    let speed = super::monster_anim::rate_of(u, inputs, |s| u.stats.get(&s).copied().unwrap_or(0));
    u.speed = Some(speed);
    if let Some(v) = with_ctx(w, inputs, key, |t, c, _| mode_velocity(t, c, ME, mode)).flatten() {
        if let Some(p) = w.objclient.c_paths.get_mut(&key) {
            set_velocity(p, v);
        }
    }
}

/// The neutral fallback F `0x004AE1D0` (§19 r4): a monster in mode 1…15
/// other than 12 → path stop and mode set 1; any other mode: nothing.
fn neutral(w: &mut ClientWorld, inputs: &ModelInputs, key: UnitKey) {
    let Some(u) = w.objclient.set_c.get(&key) else {
        return;
    };
    if !(1..=15).contains(&u.mode) || u.mode == 12 {
        return;
    }
    if let Some(p) = w.objclient.c_paths.get_mut(&key) {
        p.point_count = 0;
    }
    mode_set(w, inputs, key, 1, false);
}

/// W(f) (§19 r4): flag-ex 0x2000 := f and path +0x38 := 0.
fn walk_flag(w: &mut ClientWorld, key: UnitKey, f: bool) {
    if let Some(u) = w.objclient.set_c.get_mut(&key) {
        if f {
            u.flag_ex |= WALK_FLAG;
        } else {
            u.flag_ex &= !WALK_FLAG;
        }
    }
    if let Some(p) = w.objclient.c_paths.get_mut(&key) {
        p.field_38 = 0;
    }
}

/// The path cell of a C monster (`0x006488C0` / `0x00648900`).
pub fn cell(w: &ClientWorld, key: UnitKey) -> Option<(i32, i32)> {
    w.objclient.c_paths.get(&key).map(|p| (p.x(), p.y()))
}

/// The path direction (+0x64) of a C monster: the direction it draws
/// with.
pub fn direction(w: &ClientWorld, key: UnitKey) -> Option<u8> {
    w.objclient.c_paths.get(&key).map(|p| p.direction)
}

/// Code 0x07 at the unit's own position (`0x0046CB40`, §19 r4): W(0);
/// |Δ| ≤ 1 on both axes → the neutral fallback.
pub fn idle(w: &mut ClientWorld, inputs: &ModelInputs, key: UnitKey) {
    walk_flag(w, key, false);
    neutral(w, inputs, key);
}

/// Code 0x01, walk to the point (x, y) (§19 r4): W(1); target := the
/// point (`0x00648AD0`); compute `0x00649970` with town access = unit
/// flag 0x200000 (set on every client-only unit); no point → the neutral
/// fallback, else mode set 2. Then the tail (§19 r6): r4 ≠ 0 and the
/// stat-67 total ≠ r4 → stat 67 := r4, the rate again.
pub fn walk_to(w: &mut ClientWorld, inputs: &ModelInputs, key: UnitKey, x: i32, y: i32, r4: i32) {
    walk_flag(w, key, true);
    let n = with_ctx(w, inputs, key, |t, c, p| {
        p.set_target_point(x as u16, y as u16);
        let n = compute(t, c, p, ME, true).unwrap_or(0);
        c.foot = footprint_of(p);
        n
    });
    if n.unwrap_or(0) == 0 {
        neutral(w, inputs, key);
    } else {
        mode_set(w, inputs, key, 2, false);
    }
    tail(w, inputs, key, r4);
}

/// A point-group code (§19 r4) setting `mode`: W(0), target := none,
/// mode set, then the tail.
pub fn point_mode(w: &mut ClientWorld, inputs: &ModelInputs, key: UnitKey, mode: u32, r4: i32) {
    walk_flag(w, key, false);
    if let Some(p) = w.objclient.c_paths.get_mut(&key) {
        p.target_unit = None;
    }
    mode_set(w, inputs, key, mode, false);
    tail(w, inputs, key, r4);
}

fn tail(w: &mut ClientWorld, inputs: &ModelInputs, key: UnitKey, r4: i32) {
    if r4 == 0 {
        return;
    }
    let Some(u) = w.objclient.set_c.get_mut(&key) else {
        return;
    };
    if u.stats.get(&STAT_VELOCITYPERCENT).copied().unwrap_or(0) == r4 {
        return;
    }
    u.stats.insert(STAT_VELOCITYPERCENT, r4);
    let speed = super::monster_anim::rate_of(u, inputs, |s| u.stats.get(&s).copied().unwrap_or(0));
    u.speed = Some(speed);
    let mode = u.mode;
    if let Some(v) = with_ctx(w, inputs, key, |t, c, _| mode_velocity(t, c, ME, mode)).flatten() {
        if let Some(p) = w.objclient.c_paths.get_mut(&key) {
            set_velocity(p, v);
        }
    }
}

/// The monster update `0x004B13A0` of a C monster (§19 r8) for the modes
/// whose mode record (r8.1, normal table) has path kind 1 and end kind 1
/// (2 WL, 15 RN): the path step `0x004807C0` (P := the movement ended),
/// the anim step (kind 0), the turn `0x00648640`; P → the mode end of a
/// client-only unit (r8.7 default: mode set 1 with restart). Every other
/// mode: the anim step and the turn.
/// PROVISIONAL (client/model.md §19 r8; REC-503): the other modes take
/// anim kind 0 and no end (as `monster_anim::step`); the critters of
/// the arrival use only NU and WL.
pub fn update(w: &mut ClientWorld, inputs: &ModelInputs, key: UnitKey) {
    let Some(mode) = w.objclient.set_c.get(&key).map(|u| u.mode) else {
        return;
    };
    let walking = matches!(mode, 2 | 15);
    let mut ended = false;
    if walking {
        ended = with_ctx(w, inputs, key, |t, c, p| {
            let mut walk = Walk { t, c };
            !walk.movement(ME, p, BASE).unwrap_or(false)
        })
        .unwrap_or(false);
        sync_position(w, key);
    }
    if let Some(u) = w.objclient.set_c.get_mut(&key) {
        anim_advance(u);
    }
    if let Some(p) = w.objclient.c_paths.get_mut(&key) {
        turn(p);
    }
    if ended {
        mode_set(w, inputs, key, 1, true);
    }
}

/// Anim kind 0 (`0x00623E00`): frame += speed, wrapping at the count.
fn anim_advance(u: &mut ClientUnit) {
    let speed = u.speed.unwrap_or(0);
    u.frame = u.frame.wrapping_add(speed);
    if u.frame_count > 0 && u.frame >= u.frame_count {
        u.frame = u.frame.rem_euclid(u.frame_count);
    }
}

/// The turn `0x00648640` (`sim/pathing.md` §8.5): with direction ≠ new
/// direction and path flag 0x40 clear, direction := (direction + step)
/// & 63; once that passes the new direction (the distance left, mod 64,
/// on the far side of the step's sign), direction := new direction.
fn turn(p: &mut DynamicPath) {
    if p.direction == p.new_direction || p.flags & 0x40 != 0 {
        return;
    }
    let step = p.turn_step;
    let d = step.wrapping_add(p.direction) & 63;
    p.direction = d;
    let left = d.wrapping_sub(p.new_direction) & 63;
    // A step byte above 63 turns down (negative), else up.
    if (left > 31 && step > 63) || (left < 31 && step < 63) {
        p.direction = p.new_direction;
    }
}

/// Writes the path's cell and precise position to the model (the cell is
/// the unit's position; the draw uses the precise point) and files the
/// unit in the room of its cell.
fn sync_position(w: &mut ClientWorld, key: UnitKey) {
    let Some(p) = w.objclient.c_paths.get(&key) else {
        return;
    };
    let (cx, cy) = (p.x(), p.y());
    let precise = (p.precise_x, p.precise_y);
    let Some(u) = w.objclient.set_c.get_mut(&key) else {
        return;
    };
    let old = u.position;
    u.position = Some((cx as u16, cy as u16));
    u.precise = Some(precise);
    if old != u.position {
        let room = w.room_at(cx as u16, cy as u16).map(|r| r.room);
        w.room_units.place(super::world::view_key(key), room);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn path(direction: u8, new: u8, step: i8) -> DynamicPath {
        DynamicPath {
            direction,
            new_direction: new,
            turn_step: step as u8,
            ..DynamicPath::default()
        }
    }

    /// The recorded chicken 93 of `draws-town-arrival-ama`: 15 → 0 in
    /// steps of −4 (15, 11, 7, 3, then snapped to 0).
    #[test]
    fn a_turn_steps_toward_the_new_direction_and_snaps_when_it_passes() {
        let mut p = path(15, 0, -4);
        let mut seen = Vec::new();
        for _ in 0..5 {
            turn(&mut p);
            seen.push(p.direction);
        }
        assert_eq!(seen, [11, 7, 3, 0, 0]);
    }

    #[test]
    fn an_upward_turn_wraps_through_63() {
        let mut p = path(60, 2, 4);
        turn(&mut p);
        assert_eq!(p.direction, 0);
        turn(&mut p);
        assert_eq!(p.direction, 2);
    }

    #[test]
    fn path_flag_0x40_holds_the_direction() {
        let mut p = path(10, 20, 4);
        p.flags |= 0x40;
        turn(&mut p);
        assert_eq!(p.direction, 10);
    }
}
