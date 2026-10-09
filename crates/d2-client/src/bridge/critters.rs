// Spec: specs/client/model.md (§5 rules 6.1–6.4), specs/monsters/population.md (§11.7)
//! The client room pass `0x0044C750` (`model.md` §5 r6): at the start
//! of the client update, before the unit update, each room of the client
//! act's active-room list (list order) whose flags bit 0 is clear gets
//! its critters (`0x0046C460`, `population.md` §11.7), then its client
//! presets (`0x00466820`, r6.2), then bit 0 := 1. Every unit is made by
//! the client creator ([`super::objects::create_client_unit`]), so the
//! pass sets the client GUIDs of the arrival. The critter AI
//! `0x0046D780` (r6.4) runs in the C monsters' walk ([`c_monsters`]).
//!
//! Not run: the level-8 pass `0x0046BE60` (with `[0x007A745C]` set);
//! d2rs has no model of that flag (`docs/handoff/q-fix-p6-client-arrival-guids.md`).

use d2_sim::rng::Seed;

use super::drlg::DrlgRoomId;
use super::objects::create_client_unit;
use super::world::{
    ClientUnit, ClientWorld, KindData, ModelInputs, MonsterData, UnitKey, MONSTER, PLAYER,
};

/// The size-query mask of the critter placement (§11.7 r3).
const PLACE_MASK: u16 = 0x3F11;
/// The footprint mask of a monster (`sim/path-placement.md` §3).
const MONSTER_FOOTPRINT: u16 = 0x100;

/// The room pass `0x0044C750` (`model.md` §5 r6). No client DRLG:
/// nothing.
pub fn room_pass(w: &mut ClientWorld, inputs: &ModelInputs) {
    let Some(d) = w.drlg.as_ref() else {
        return;
    };
    let rooms = d.active_rooms();
    if rooms.iter().all(|r| d.populated(r.room)) {
        return;
    }
    // The footprints the placement test reads (PROVISIONAL REC-546).
    super::client_missiles::stamp_unit_footprints_with(
        w,
        &inputs.tables.monsters,
        &inputs.objclient.rows,
    );
    for r in rooms {
        let Some(d) = w.drlg.as_ref() else {
            return;
        };
        if d.populated(r.room) {
            continue;
        }
        critters(w, inputs, r.room, r.level);
        presets(w, r.room);
        if let Some(d) = w.drlg.as_mut() {
            d.set_populated(r.room);
        }
    }
}

/// One room seed draw `rand(n)` (`0x0045C3E0`, §11.7 r2): n < 1 → 0
/// with no draw.
fn rand(w: &mut ClientWorld, room: DrlgRoomId, n: i32) -> i32 {
    w.drlg
        .as_mut()
        .and_then(|d| d.room_seed(room))
        .map_or(0, |s| s.roll(n) as i32)
}

/// The critter pass `0x0046C460(room)` (§11.7 r2).
fn critters(w: &mut ClientWorld, inputs: &ModelInputs, room: DrlgRoomId, level: u16) {
    let Some(l) = inputs.tables.levels.get(usize::from(level)) else {
        return;
    };
    let c = l.critters;
    for i in 0..4 {
        let class = c.cmon[i];
        if class < 0 {
            return;
        }
        // `pct()`: one step, lo % 100 (unsigned), drawn for every slot.
        if rand(w, room, 100) >= i32::from(c.cpct[i]) {
            continue;
        }
        let row = inputs
            .tables
            .monsters
            .get(class as usize)
            .copied()
            .flatten();
        let (min, max) = row.map_or((0, 0), |m| (i32::from(m.min_grp), i32::from(m.max_grp)));
        let mut n = rand(w, room, max - min) + min;
        if c.camt[i] != 0 {
            n *= i32::from(c.camt[i]);
        }
        for _ in 0..n {
            place_critter(w, inputs, room, class);
        }
    }
}

/// `place_critter` `0x0046C1A0(room, c)` (§11.7 r3): up to 10 random
/// points of the room (x drawn first); the first free one gets the
/// critter. 10 occupied points: no critter and no GUID.
fn place_critter(w: &mut ClientWorld, inputs: &ModelInputs, room: DrlgRoomId, class: i16) {
    let Some(row) = inputs
        .tables
        .monsters
        .get(class as usize)
        .copied()
        .flatten()
    else {
        return;
    };
    let size = i32::from(row.size_x);
    let Some(d) = w.drlg.as_ref() else {
        return;
    };
    let Some(a) = d.drlg.active_room(room) else {
        return;
    };
    let (id, r) = (a.id, a.subtiles);
    for _ in 0..10 {
        let x = r.x + rand(w, room, r.w - 1);
        let y = r.y + rand(w, room, r.h - 1);
        if super::client_missiles::size_query(w, id, x, y, size, PLACE_MASK) != 0 {
            continue;
        }
        let Some(key) = create_client_unit(w, MONSTER, class as u32, x as u16, y as u16) else {
            return;
        };
        // The 0xAC create of r6.3: mode 1 (NU).
        if let Some(u) = w.objclient.set_c.get_mut(&key) {
            u.mode = 1;
        }
        let town = (row.npc || row.in_town) && !row.interact;
        let pattern = super::client_missiles::footprint_pattern(size, town);
        super::client_missiles::stamp_footprint(w, x, y, pattern, MONSTER_FOOTPRINT);
        return;
    }
}

/// The client presets `0x00466820(room)` (`model.md` §5 r6): each
/// preset unit of the room with flag bit 0, in list order, at its
/// position plus the room's sub-tile origin, with its type and mode.
fn presets(w: &mut ClientWorld, room: DrlgRoomId) {
    let Some(d) = w.drlg.as_ref() else {
        return;
    };
    let Some(a) = d.drlg.active_room(room) else {
        return;
    };
    let (x0, y0) = (a.subtiles.x, a.subtiles.y);
    let list = d.client_presets(room);
    for p in list {
        let (x, y) = ((p.x + x0) as u16, (p.y + y0) as u16);
        let Some(key) = create_client_unit(w, p.unit_type as u8, p.class as u32, x, y) else {
            continue;
        };
        if let Some(u) = w.objclient.set_c.get_mut(&key) {
            u.mode = p.mode;
        }
    }
}

/// The C monsters' walk (`model.md` §5 r3, after the S units): each
/// still in set C runs the critter AI `0x0046D780` (r6.4).
pub fn c_monsters(w: &mut ClientWorld, inputs: &ModelInputs) {
    for key in super::objects::c_order(w, MONSTER) {
        if w.objclient.set_c.contains_key(&key) {
            critter_ai(w, inputs, key);
        }
    }
}

/// Monster mode 0 and 12 are the dead modes the slot scan skips (r6.4,
/// `0x0046C720`).
fn alive_player(u: &ClientUnit) -> bool {
    u.mode != 0 && u.mode != 12 && u.position.is_some()
}

/// The monster size of a class (`monstats2` `SizeX`); 0 without a row.
fn monster_size(inputs: &ModelInputs, class: u32) -> i32 {
    inputs
        .tables
        .monsters
        .get(class as usize)
        .and_then(|c| c.as_ref())
        .map_or(0, |c| i32::from(c.size_x))
}

fn think_mut(w: &mut ClientWorld, key: UnitKey) -> Option<&mut MonsterData> {
    match &mut w.objclient.set_c.get_mut(&key)?.kind {
        KindData::Monster(d) => Some(d),
        _ => None,
    }
}

/// One step of U's seed (+0x20); a unit without a seed reads 0.
fn unit_step(w: &mut ClientWorld, key: UnitKey) -> u32 {
    let Some(u) = w.objclient.set_c.get_mut(&key) else {
        return 0;
    };
    let Some((lo, hi)) = u.seed else {
        return 0;
    };
    let mut s = Seed::new(lo, hi);
    let r = s.step();
    u.seed = Some((s.lo, s.hi));
    r
}

/// The critter AI `0x0046D780(U)` (`model.md` §5 r6).
fn critter_ai(w: &mut ClientWorld, inputs: &ModelInputs, key: UnitKey) {
    let Some(d) = think_mut(w, key) else {
        return;
    };
    let t = d.think;
    d.first_think.get_or_insert(t);
    if t > 0 {
        d.think = t - 1;
        return;
    }
    let u = &w.objclient.set_c[&key];
    let class = u.class;
    let size = monster_size(inputs, class);
    // P, D: the nearest of the player slots (first one on a tie).
    let mut nearest: Option<i32> = None;
    for (k, p) in &w.units {
        if k.unit_type != PLAYER || !alive_player(p) {
            continue;
        }
        let dist = super::objects::distance(u, size, p, 2);
        if nearest.is_none_or(|n| dist < n) {
            nearest = Some(dist);
        }
    }
    let dist = match nearest {
        Some(dist) => dist,
        None if w.local().is_none() => {
            set_think(w, key, 25);
            request(w, key, Request::Idle);
            return;
        }
        None => i32::MAX,
    };
    if dist > 30 {
        set_think(w, key, (dist - 15).min(200));
        request(w, key, Request::Idle);
        return;
    }
    match class {
        CHICKEN | BUG => chicken_ai(w, inputs, key),
        // PROVISIONAL (client/model.md §5 r6; REC-741): the zoo test
        // (`monstats` flag 22, `0x0046D660`) and the other class bodies
        // are not modelled; they run nothing.
        151 | 269 | 283 | 339 | 157 | 158 | 319 | 159 | 227 | 318 | 556 | 574 | 278..=282 => {}
        _ => request(w, key, Request::Idle),
    }
}

const CHICKEN: u32 = 149;
const BUG: u32 = 268;

fn set_think(w: &mut ClientWorld, key: UnitKey, t: i32) {
    if let Some(d) = think_mut(w, key) {
        d.think = t;
    }
}

/// A critter's mode request (`model.md` §19 r4).
enum Request {
    /// Code 7 at U's own position (`0x0046CB40`): the neutral fallback.
    Idle,
    /// Code 1 (walk to a point, mode 2) or 0x0C (mode 8 S1); the point
    /// is the path's, not modelled (REC-742).
    To { code: u8 },
}

/// The request on a C monster. PROVISIONAL (client/model.md §5 r6,
/// §19; REC-742): the client path of a C monster is not modelled; the
/// request sets the mode its code sets (code 1 → 2 WL, 0x0C → 8 S1; code
/// 7 at the own position: modes 1…15 other than 12 → 1) and the unit
/// keeps its cell.
fn request(w: &mut ClientWorld, key: UnitKey, r: Request) {
    let Some(u) = w.objclient.set_c.get_mut(&key) else {
        return;
    };
    match r {
        Request::Idle => {
            if (1..=15).contains(&u.mode) && u.mode != 12 {
                u.mode = 1;
            }
        }
        Request::To { code, .. } => {
            u.mode = if code == 1 { 2 } else { 8 };
        }
    }
}

/// `step(d, code, r4)` `0x0046C960`: two draws of U's seed (x ± d by
/// bit 0 of the first, y ± d by the second), then the request; the
/// point goes to the path, which is not modelled (REC-742).
fn step(w: &mut ClientWorld, key: UnitKey, code: u8) {
    unit_step(w, key);
    unit_step(w, key);
    request(w, key, Request::To { code });
}

/// `chicken_ai(U)` `0x0046CCD0` (`model.md` §5 r6).
fn chicken_ai(w: &mut ClientWorld, inputs: &ModelInputs, key: UnitKey) {
    let u = &w.objclient.set_c[&key];
    if u.mode != 1 {
        return;
    }
    let size = monster_size(inputs, u.class);
    let Some((v, dist)) = nearest_near(w, inputs, key, size) else {
        return;
    };
    if dist < 4 {
        // Flee from V (`0x0046C7D0`), move mask 0xC01.
        let (ux, uy) = cell_i32(u.cell());
        let s = ((ux - v.0).signum(), (uy - v.1).signum());
        let room = w.room_at(u.cell().0, u.cell().1);
        let id = room.and_then(|r| {
            let d = w.drlg.as_ref()?;
            d.drlg.active_room(r.room).map(|a| a.id)
        });
        let free = id.is_some_and(|id| {
            super::client_missiles::size_query(w, id, ux + s.0, uy + s.1, 1, 0xC01) == 0
        });
        // Free: to U + 4s; else step(4, 0x0C, 120).
        if free {
            request(w, key, Request::To { code: 0x0C });
        } else {
            step(w, key, 0x0C);
        }
    } else if unit_step(w, key) % 100 < 30 {
        // step(2, 1, 0): walk.
        step(w, key, 1);
    } else {
        set_think(w, key, 5);
    }
}

fn cell_i32((x, y): (u16, u16)) -> (i32, i32) {
    (i32::from(x), i32::from(y))
}

/// `0x0046C570` with `0x0046C600`: the nearest unit in the rooms near U
/// (U's room's adjacency array): a player, or a monster not class 149
/// or 556, alive, not U; path distance (first one on a tie, S units by
/// key, then set C). Returns its cell and the distance.
fn nearest_near(
    w: &ClientWorld,
    inputs: &ModelInputs,
    key: UnitKey,
    size: i32,
) -> Option<((i32, i32), i32)> {
    let u = &w.objclient.set_c[&key];
    let d = w.drlg.as_ref()?;
    let room = w.room_at(u.cell().0, u.cell().1)?;
    let rects: Vec<_> = d
        .adjacency(room.room)
        .into_iter()
        .filter_map(|r| d.drlg.active_room(r).map(|a| a.subtiles))
        .collect();
    let mut best: Option<((i32, i32), i32)> = None;
    let units = w.units.iter().chain(w.objclient.set_c.iter());
    for (k, o) in units {
        if *k == key || o.is_dead() {
            continue;
        }
        let other_size = match k.unit_type {
            PLAYER => 2,
            MONSTER if !matches!(o.class, CHICKEN | 556) => monster_size(inputs, o.class),
            _ => continue,
        };
        let Some(p) = o.position else {
            continue;
        };
        let (px, py) = cell_i32(p);
        if !rects.iter().any(|r| r.contains(px, py)) {
            continue;
        }
        let dist = super::objects::distance(u, size, o, other_size);
        if best.is_none_or(|(_, b)| dist < b) {
            best = Some(((px, py), dist));
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bridge::world::MonsterClass;

    fn inputs() -> ModelInputs {
        let mut i = ModelInputs::default();
        i.tables.monsters = vec![None; 150];
        i.tables.monsters[149] = Some(MonsterClass {
            size_x: 1,
            ..MonsterClass::default()
        });
        i
    }

    /// A world with one chicken in set C at (100, 100) and, when
    /// `player` is given, the local player there.
    fn world(player: Option<(u16, u16)>) -> (ClientWorld, UnitKey) {
        let mut w = ClientWorld::default();
        let key = create_client_unit(&mut w, MONSTER, CHICKEN, 100, 100).unwrap();
        w.objclient.set_c.get_mut(&key).unwrap().mode = 1;
        if let Some(p) = player {
            let pk = UnitKey::new(PLAYER, 1);
            let mut u = ClientUnit::new(pk);
            u.position = Some(p);
            u.mode = 1;
            w.units.insert(pk, u);
            w.local_player = Some(pk);
        }
        (w, key)
    }

    fn think(w: &ClientWorld, key: UnitKey) -> (i32, Option<i32>) {
        match &w.objclient.set_c[&key].kind {
            KindData::Monster(d) => (d.think, d.first_think),
            _ => unreachable!(),
        }
    }

    // Covers: specs/client/model.md §5 r6
    #[test]
    fn a_new_critter_reads_t_zero_and_a_running_timer_counts_down() {
        let (mut w, key) = world(None);
        let i = inputs();
        // No player and no local player: T = 25, idle (mode 1 stays).
        c_monsters(&mut w, &i);
        assert_eq!(think(&w, key), (25, Some(0)));
        c_monsters(&mut w, &i);
        assert_eq!(think(&w, key), (24, Some(0)));
        assert_eq!(w.objclient.set_c[&key].mode, 1);
    }

    // Covers: specs/client/model.md §5 r6
    #[test]
    fn a_far_player_sets_the_timer_to_the_distance_less_15_at_most_200() {
        let i = inputs();
        // Δ = 40 on x: (2·(40 − 1) + 0) / 2 = 39 > 30 → T = 24.
        let (mut w, key) = world(Some((140, 100)));
        c_monsters(&mut w, &i);
        assert_eq!(think(&w, key).0, 24);
        let (mut w, key) = world(Some((1000, 100)));
        c_monsters(&mut w, &i);
        assert_eq!(think(&w, key).0, 200);
        // Near (no client DRLG: no unit near U): nothing, no seed draw.
        let (mut w, key) = world(Some((110, 100)));
        let seed = w.objclient.set_c[&key].seed;
        c_monsters(&mut w, &i);
        assert_eq!(think(&w, key).0, 0);
        assert_eq!(w.objclient.set_c[&key].seed, seed);
    }

    // Covers: specs/client/model.md §5 r6
    #[test]
    fn without_a_client_drlg_the_room_pass_makes_nothing() {
        let mut w = ClientWorld::default();
        room_pass(&mut w, &inputs());
        assert!(w.objclient.set_c.is_empty());
        assert_eq!(w.objclient.next_guid, 1);
    }
}
