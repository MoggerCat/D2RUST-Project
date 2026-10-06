// Spec: specs/monsters/population.md §3, §4, §5, §12
//! Room population (§3, `0x0054EC90`), the monster pick (§4,
//! `0x005BDE80`), boss or pack (§5, `0x005BE020`) and ambient spawns (§12,
//! `0x0054F060`).

use super::placement::{place_at, spawn_point};
use super::region::Region;
use super::seams::PopHost;
use super::spawn::{champion_minions, pack, random_boss};
use super::{CoordRect, Ctx};
use crate::rng::Seed;
use crate::units::RoomId;

/// Density modulus (§3.2).
pub const DENSITY_MOD: u32 = 100_000;
/// MonDen clamp (§3.1).
pub const MON_DEN_MAX: i32 = 10_000;
/// Placespawn chance of the normal pick (§3.2).
pub const PICK_CHANCE: u32 = 20;
/// Level 108 (Chaos Sanctum, §3.1 step 6).
pub const CHAOS_SANCTUM: i32 = 108;
/// Wanderer list `0x00731B2C` and per-act table `0x00731B30` (§12).
pub const WANDERERS: [i32; 1] = [270];
pub const WANDERER_ACTS: [(u8, u8); 5] = [(0, 1), (0, 0), (0, 0), (0, 0), (0, 0)];
/// Wanderers per level per game (§12 step 4).
pub const MAX_WANDERERS: i32 = 3;

/// The §4 pick result: the class and whether its monstats record exists.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pick {
    pub class: i32,
    pub record: bool,
}

/// §4 `0x005BDE80(game, region, room, &record, chance, umon)` for the
/// region of `level`. Draws on the room seed.
pub fn pick<H: PopHost + ?Sized>(
    cx: &mut Ctx<'_, H>,
    level: i32,
    room: RoomId,
    chance: u32,
    umon: bool,
) -> Pick {
    let t = cx.tables;
    let none = Pick {
        class: 0,
        record: false,
    };
    let Some(region) = cx.state.regions.get(level) else {
        // TODO(spec: population.md §4): a null region is not described.
        return none;
    };
    let seed = cx.host.room_seed(room);
    if umon && cx.info.difficulty == 0 {
        let list = t.level(region.level_id).map_or(&[][..], |l| &l.umon[..]);
        if list.is_empty() {
            return none;
        }
        let class = i32::from(list[seed.roll(list.len() as i32) as usize]);
        return Pick {
            class,
            record: t.mon(class).is_some(),
        };
    }
    pick_region(t, region, seed, chance)
}

/// §4 steps 2–5: the rarity walk and the placespawn swap.
pub fn pick_region(t: &super::PopTables, region: &Region, seed: &mut Seed, chance: u32) -> Pick {
    if region.mon_count == 0 {
        return Pick {
            class: 0,
            record: false,
        };
    }
    let mut w = seed.roll(i32::from(region.total_rarity)) as i32 + 1;
    let n = usize::from(region.mon_count);
    let mut i = 0;
    while i < n {
        w -= i32::from(region.entries.get(i).map_or(0, |e| e.rarity));
        if w <= 0 {
            break;
        }
        i += 1;
    }
    let mut class = region.entry_class(i);
    if let Some(m) = t.mon(class) {
        if m.spawn >= 0 && m.place_spawn && seed.step() % 100 > chance {
            class = i32::from(m.spawn);
        }
    }
    Pick {
        class,
        record: t.mon(class).is_some(),
    }
}

/// §5 `0x005BE020(region, room)`: 0 boss, 1 or 2 pack (raw result).
pub fn boss_or_pack(region: &Region, seed: &mut Seed) -> u8 {
    let u = region.bosses as u8;
    let v = region.rooms_visited;
    let n = region.room_count;
    if u < region.mon_umin && n != 0 {
        let d = (seed.step() % 100) as i32;
        if d < v.wrapping_mul(100) / n {
            return 0;
        }
    }
    if u < region.mon_umax && seed.step() % 100 <= 5 {
        return 0;
    }
    if seed.step() % 100 > 35 {
        2
    } else {
        1
    }
}

/// §3.2 step 2: tries of a rectangle in subtiles.
pub fn tries(sub: [i32; 4]) -> i32 {
    let [l, t, r, b] = sub;
    (b.wrapping_sub(t) / 3).wrapping_mul(r.wrapping_sub(l) / 3)
}

/// §3 `0x0054EC90(game, room)`: the first monster population of a room.
pub fn populate_room<H: PopHost + ?Sized>(cx: &mut Ctx<'_, H>, room: RoomId) {
    // §3.1 guard `0x0054EBC0`.
    let lvl0 = cx.host.room_level(room);
    let Some(guard) = cx.state.regions.get_mut(lvl0) else {
        return;
    };
    guard.rooms_visited = guard.rooms_visited.wrapping_add(1);
    let lvl = cx.host.populated_level(room);
    if lvl == 0 {
        return;
    }
    if guard.room_count < 0 {
        let act = guard.act;
        let n = cx.host.populated_room_count(act, lvl);
        if let Some(g) = cx.state.regions.get_mut(lvl0) {
            g.room_count = n;
        }
    }
    let Some(guard) = cx.state.regions.get(lvl0) else {
        return;
    };
    if guard.mon_den == 0 || guard.room_count == 0 {
        return;
    }
    if lvl == CHAOS_SANCTUM && cx.host.chaos_blocks_population() {
        return;
    }
    let list = cx.host.coord_list(room);
    if list.is_empty() {
        return;
    }
    let Some(region) = cx.state.regions.get_mut(lvl) else {
        // TODO(spec: population.md §3.1): regions[lvl] null after the
        // guard passed is not described; nothing is populated.
        return;
    };
    if region.mon_den > MON_DEN_MAX {
        region.mon_den = MON_DEN_MAX;
    }
    let mut spawned = false;
    for r in list {
        if !populate_rect(cx, room, lvl, r, &mut spawned) {
            // §3.2 step 3.2: a failed pick ends the room, §3.4 skipped.
            return;
        }
    }
    if spawned {
        if let Some(region) = cx.state.regions.get_mut(lvl) {
            region.rooms_with_spawns = region.rooms_with_spawns.wrapping_add(1);
        }
    }
}

/// §3.2 for one rectangle. Returns false when a pick failed.
fn populate_rect<H: PopHost + ?Sized>(
    cx: &mut Ctx<'_, H>,
    room: RoomId,
    lvl: i32,
    r: CoordRect,
    spawned: &mut bool,
) -> bool {
    if r.index == 0 || r.node_flag != 0 || (r.rect[0] == 0 && r.rect[2] == 0) {
        return true;
    }
    for _ in 0..tries(r.subtiles()).max(0) {
        let mon_den = cx.state.regions.get(lvl).map_or(0, |g| g.mon_den);
        let d = cx.host.game_seed().step() % DENSITY_MOD;
        if i64::from(d) > i64::from(mon_den) {
            continue;
        }
        let p = pick(cx, lvl, room, PICK_CHANCE, false);
        if !p.record {
            return false;
        }
        let kind = match cx.state.regions.get(lvl) {
            Some(region) => {
                let region = region.clone();
                boss_or_pack(&region, cx.host.room_seed(room))
            }
            None => 2,
        };
        // §3.2 step 3.3: 1 becomes 2 (§3.3: the t = 1 branch is dead).
        if kind == 0 {
            let b = pick(cx, lvl, room, 0, true);
            if let Some(boss) = random_boss(cx, room, Some(r), b.class, true, 0, 0, true) {
                *spawned = true;
                champion_minions(cx, Some(r), boss, b.class);
            }
        } else if pack(cx, room, r, p.class).is_some() {
            *spawned = true;
        }
    }
    true
}

/// §12 `0x0054F060(game, room)`: the ambient (wandering) spawn, every
/// tick for every active room.
pub fn ambient<H: PopHost + ?Sized>(cx: &mut Ctx<'_, H>, room: RoomId) {
    if cx.host.room_seed(room).step() & 0x7FFF != 0 {
        return;
    }
    if cx.host.client_count(room) != 0 {
        return;
    }
    let level = cx.host.room_level(room);
    let Some(l) = cx.tables.level(level) else {
        return;
    };
    if l.mon_wndr == 0 {
        return;
    }
    // `0x0054EFF0`.
    if cx.host.room_seed(room).step() % 100 >= 3 {
        return;
    }
    if cx
        .state
        .regions
        .get(level)
        .is_none_or(|g| g.wanderers >= MAX_WANDERERS)
    {
        return;
    }
    // `0x0054EF50`.
    let Some(&(first, count)) = WANDERER_ACTS.get(usize::from(l.act)) else {
        return;
    };
    if count == 0 {
        return;
    }
    let i = cx.host.room_seed(room).roll(i32::from(count)) as usize + usize::from(first);
    let class = WANDERERS[i];
    let Some((x, y)) = spawn_point(cx, room, None, class, false) else {
        return;
    };
    if let Some(u) = place_at(cx, room, None, x, y, class, 1, -1, 0).unit() {
        cx.host.change_alignment(u, 0, 8);
        if let Some(g) = cx.state.regions.get_mut(level) {
            g.wanderers = g.wanderers.wrapping_add(1);
        }
    }
}
