// Spec: specs/world/objects-2.md §16–§18 (Test vectors)
use super::super::fake::{blank_object, Call, Fake};
use super::super::{dispatch, object_event, oevent, oflags, Dispatch, ObjectData, Operator};
use super::*;
use crate::rng::Seed;

/// Every part-2 seam call is recorded as `Call::Other`.
impl MechWorld for Fake {
    fn warp_through_tile(&mut self, player: UnitId, tile: UnitId) {
        self.calls
            .push(Call::Other(format!("warp {} {}", player.0, tile.0)));
    }
    fn set_interact(&mut self, player: UnitId, unit_type: u8, guid: u32) {
        self.calls.push(Call::Other(format!(
            "interact {} {unit_type} {guid:#x}",
            player.0
        )));
    }
    fn has_gem(&self, player: UnitId) -> bool {
        self.keys.contains(&player)
    }
    fn send(&mut self, player: UnitId, msg: &[u8]) {
        self.calls
            .push(Call::Other(format!("send {} {msg:02x?}", player.0)));
    }
    fn stand_drop(&mut self, object: UnitId, weapon: bool) {
        self.calls
            .push(Call::Other(format!("stand {} {weapon}", object.0)));
    }
    fn drop_code_quality(&mut self, object: UnitId, code: u32, quality: u8) {
        let s = String::from_utf8_lossy(&code.to_le_bytes()).into_owned();
        self.calls
            .push(Call::Other(format!("dropcode {} {s} q{quality}", object.0)));
    }
    fn trap_damage_arg(&mut self, object: UnitId, target: UnitId, arg: u32) {
        self.calls.push(Call::Other(format!(
            "damage {} {} {arg}",
            object.0, target.0
        )));
    }
    fn in_town(&self, _room: RoomId) -> bool {
        true
    }
    /// The point itself in the same room.
    fn free_point(
        &self,
        room: RoomId,
        x: i32,
        y: i32,
        _size: i32,
        _mask: u32,
    ) -> Option<(RoomId, i32, i32)> {
        Some((room, x, y))
    }
    fn place_unit(&mut self, unit: UnitId, room: RoomId, x: i32, y: i32) -> bool {
        self.calls
            .push(Call::Other(format!("place {} {} {x} {y}", unit.0, room.0)));
        true
    }
    fn recount_tomes(&mut self, player: UnitId) {
        self.calls
            .push(Call::Other(format!("recount {}", player.0)));
    }
}

const O: UnitId = UnitId(10);
const P: UnitId = UnitId(20);
const ROOM: RoomId = RoomId(3);
const CLASS: u16 = 5;

/// One `Sync` row (no unit-seed draw), `FrameCnt1` 10 · 256.
fn tables() -> ObjectTables {
    let mut o = blank_object();
    o.sync = 1;
    o.framecnt1 = 10 << 8;
    o.framecnt3 = 4 << 8;
    o.mode2 = 1;
    o.selectable0 = 1;
    ObjectTables {
        objects: vec![o; 400],
        ..Default::default()
    }
}

fn setup(class: u16, mode: u8, fn_: u8, t: &mut ObjectTables) -> (ObjectControl, Fake) {
    t.objects[class as usize].operatefn = fn_;
    let mut ctl = ObjectControl {
        seed: Seed::new(1, 666),
        regions: Vec::new(),
        shrine_lists: Default::default(),
        data: Default::default(),
    };
    ctl.data.insert(
        O,
        ObjectData {
            class,
            ..Default::default()
        },
    );
    let mut f = Fake {
        frame: 100,
        ..Fake::default()
    };
    f.modes.insert(O, mode);
    f.rooms.insert(O, ROOM);
    f.rooms.insert(P, ROOM);
    f.flags.insert(O, oflags::SELECTABLE);
    f.operators.insert(P, Operator::Player(1));
    f.guids.insert(O, 0x55);
    f.guids.insert(P, 0x77);
    (ctl, f)
}

fn others(f: &Fake) -> Vec<String> {
    f.calls
        .iter()
        .filter_map(|c| match c {
            Call::Other(s) => Some(s.clone()),
            _ => None,
        })
        .collect()
}

fn modes(f: &Fake) -> Vec<u8> {
    f.calls
        .iter()
        .filter_map(|c| match c {
            Call::Mode(_, m, _) => Some(*m),
            _ => None,
        })
        .collect()
}

// Covers: specs/world/objects-2.md §16.1
#[test]
fn torch_tiki_toggles() {
    let mut t = tables();
    for (from, to) in [(0u8, Some(1u8)), (1, Some(0)), (2, None)] {
        let (mut c, mut f) = setup(CLASS, from, 13, &mut t);
        assert_eq!(
            dispatch(&mut c, &t, &mut f, O, Some(P)),
            Ok(Dispatch::Done(0))
        );
        assert_eq!(modes(&f), to.into_iter().collect::<Vec<_>>());
    }
}

// Covers: specs/world/objects-2.md §16.4, §16.5
#[test]
fn secret_door_and_stand_order() {
    let mut t = tables();
    let (mut c, mut f) = setup(CLASS, 0, 18, &mut t);
    assert_eq!(
        dispatch(&mut c, &t, &mut f, O, Some(P)),
        Ok(Dispatch::Done(1))
    );
    assert_eq!(modes(&f), vec![1]);
    assert_eq!(f.flags[&O] & oflags::SELECTABLE, 0);
    assert!(f.calls.contains(&Call::Free(O)));
    assert!(f.calls.contains(&Call::Schedule(O, oevent::END_ANIM, 111)));
    // Stand: drop before the mode change.
    let (mut c, mut f) = setup(CLASS, 0, 20, &mut t);
    dispatch(&mut c, &t, &mut f, O, Some(P)).unwrap();
    let i_drop = f
        .calls
        .iter()
        .position(|c| *c == Call::Other("stand 10 true".into()))
        .unwrap();
    let i_mode = f
        .calls
        .iter()
        .position(|c| matches!(c, Call::Mode(_, 2, _)))
        .unwrap();
    assert!(i_drop < i_mode);
    // Not mode 0: nothing.
    let (mut c, mut f) = setup(CLASS, 2, 19, &mut t);
    assert_eq!(
        dispatch(&mut c, &t, &mut f, O, Some(P)),
        Ok(Dispatch::Done(1))
    );
    assert!(f.calls.is_empty());
}

// Covers: specs/world/objects-2.md §16.6
#[test]
fn bookshelf_code_draws() {
    let mut t = tables();
    let (mut c, mut f) = setup(CLASS, 0, 26, &mut t);
    let mut e = Seed::new(1, 666);
    let r = e.step() % 20;
    let b = e.step() & 1;
    // {1, 666}: lo' 1791398751 mod 20 = 11 < 13; lo' 791599131 & 1 = 1.
    assert_eq!((r, b), (11, 1));
    dispatch(&mut c, &t, &mut f, O, Some(P)).unwrap();
    assert_eq!(c.seed, e);
    assert_eq!(others(&f), vec!["dropcode 10 isc  q2".to_string()]);
    assert_eq!(c.get(O).unwrap().drop_code, u32::from_le_bytes(*b"isc "));
    assert_eq!(modes(&f), vec![2]);
}

// Covers: specs/world/objects-2.md §16.3
#[test]
fn obelisk_needs_a_gem() {
    let mut t = tables();
    let (mut c, mut f) = setup(CLASS, 0, 17, &mut t);
    dispatch(&mut c, &t, &mut f, O, Some(P)).unwrap();
    assert!(f
        .calls
        .contains(&Call::Sound(P, sound::PORTAL_REFUSED, Some(P), false)));
    assert!(modes(&f).is_empty());
    let (mut c, mut f) = setup(CLASS, 0, 17, &mut t);
    f.keys.insert(P);
    assert_eq!(
        dispatch(&mut c, &t, &mut f, O, Some(P)),
        Ok(Dispatch::Done(1))
    );
    assert_eq!(modes(&f), vec![3]);
    assert_eq!(
        others(&f),
        vec![
            "interact 20 2 0x55".to_string(),
            "send 20 [58, 55, 00, 00, 00, 00, 00]".to_string()
        ]
    );
}

// Covers: specs/world/objects-2.md §16.8, §16.9, §16.10
#[test]
fn slime_door_exploding_chest_bank() {
    let mut t = tables();
    t.objects[CLASS as usize].selectable1 = 1;
    let (mut c, mut f) = setup(CLASS, 0, 29, &mut t);
    dispatch(&mut c, &t, &mut f, O, Some(P)).unwrap();
    assert_ne!(f.flags[&O] & oflags::SELECTABLE, 0, "Selectable[1]");
    assert!(f.calls.contains(&Call::Free(O)));
    let (mut c, mut f) = setup(CLASS, 0, 30, &mut t);
    assert_eq!(
        dispatch(&mut c, &t, &mut f, O, Some(P)),
        Ok(Dispatch::Done(0))
    );
    assert_eq!(others(&f), vec!["damage 10 20 0", "damage 10 20 1"]);
    assert_eq!(modes(&f), vec![1]);
    let (mut c, mut f) = setup(BANK_CLASS, 0, 32, &mut t);
    assert_eq!(
        dispatch(&mut c, &t, &mut f, O, Some(P)),
        Ok(Dispatch::Done(0))
    );
    assert_eq!(
        others(&f),
        vec!["interact 20 2 0x55", "send 20 [77, 10]", "recount 20"]
    );
}

// Covers: specs/world/objects-2.md §16.12, §18.5
#[test]
fn jungle_stash_schedules_drop_and_keeps_owner() {
    let mut t = tables();
    t.objects[CLASS as usize].parm1 = 11;
    let (mut c, mut f) = setup(CLASS, 0, 51, &mut t);
    dispatch(&mut c, &t, &mut f, O, Some(P)).unwrap();
    assert!(f
        .calls
        .contains(&Call::Schedule(O, mevent::STASH_DROP, 112)));
    assert!(f.calls.contains(&Call::Free(O)), "HasCollision1 0");
    assert_eq!(c.get(O).unwrap().owner, Some(0x77));
}

// Covers: specs/world/objects-2.md §16.13
#[test]
fn gate_debounce_and_close() {
    let mut t = tables();
    let (mut c, mut f) = setup(CLASS, 0, 61, &mut t);
    f.tick = 1000;
    dispatch(&mut c, &t, &mut f, O, Some(P)).unwrap();
    assert_eq!(modes(&f), vec![1]);
    assert_eq!(c.get(O).unwrap().last_tick, 1000);
    f.calls.clear();
    f.modes.insert(O, 2);
    f.tick = 1499;
    dispatch(&mut c, &t, &mut f, O, Some(P)).unwrap();
    assert!(f.calls.is_empty(), "within 500");
    f.tick = 1500;
    dispatch(&mut c, &t, &mut f, O, Some(P)).unwrap();
    assert_eq!(f.calls[0], Call::Stamp(O));
    assert_eq!(modes(&f), vec![0]);
}

// Covers: specs/world/objects-2.md §18.4
#[test]
fn trapped_soul_operate_and_events() {
    let mut t = tables();
    // r = 51 < 90: D(0), no item → no mode.
    let (mut c, mut f) = setup(CLASS, 0, 48, &mut t);
    assert_eq!(
        dispatch(&mut c, &t, &mut f, O, Some(P)),
        Ok(Dispatch::Done(0))
    );
    assert!(modes(&f).is_empty());
    // Event 9 handler by mode.
    let (mut c, mut f) = setup(CLASS, 1, 48, &mut t);
    object_event(&mut c, &t, &mut f, O, mevent::SOUL).unwrap();
    assert_eq!(f.schedules(), vec![(O, mevent::SOUL, 110)]);
    let (mut c, mut f) = setup(CLASS, 3, 48, &mut t);
    object_event(&mut c, &t, &mut f, O, mevent::SOUL).unwrap();
    assert_eq!(modes(&f), vec![4]);
    // Mode 2, InteractType 0 → 1 < 2: burn (no units), event 9 at +25.
    let (mut c, mut f) = setup(CLASS, 2, 48, &mut t);
    object_event(&mut c, &t, &mut f, O, mevent::SOUL).unwrap();
    assert_eq!(c.get(O).unwrap().interact, 1);
    assert_eq!(f.schedules(), vec![(O, mevent::SOUL, 125)]);
}

// Covers: specs/world/objects-2.md §18.1, §18.6
#[test]
fn fire_event_and_endanim_write_mode_directly() {
    let mut t = tables();
    let (mut c, mut f) = setup(CLASS, 1, 0, &mut t);
    object_event(&mut c, &t, &mut f, O, mevent::FIRE).unwrap();
    assert_eq!(f.modes[&O], 2);
    assert!(modes(&f).is_empty(), "no mode set");
    let mut e = Seed::new(1, 666);
    let lo = e.step();
    assert_eq!(
        f.schedules(),
        vec![(O, mevent::FIRE, 115 + (lo % 35) as i32)]
    );
    // ENDANIM: direct write, footprint freed (HasCollision2 0), no flag.
    let (mut c, mut f) = setup(CLASS, 1, 0, &mut t);
    f.flags.insert(O, 0);
    object_event(&mut c, &t, &mut f, O, oevent::END_ANIM).unwrap();
    assert_eq!(
        f.calls,
        vec![Call::Other("store 10 2".into()), Call::Free(O)]
    );
    assert_eq!(f.flags[&O], 0);
    // Mode 2: nothing.
    let (mut c, mut f) = setup(CLASS, 2, 0, &mut t);
    object_event(&mut c, &t, &mut f, O, oevent::END_ANIM).unwrap();
    assert!(f.calls.is_empty());
}

// Covers: specs/world/objects-2.md §18.2, §18.3
#[test]
fn spike_and_fissure_events() {
    let mut t = tables();
    let (mut c, mut f) = setup(CLASS, 0, 0, &mut t);
    object_event(&mut c, &t, &mut f, O, mevent::SPIKE).unwrap();
    assert_eq!(f.schedules(), vec![(O, mevent::SPIKE, 115)], "nobody on it");
    c.get_mut(O).unwrap().interact = 2;
    f.modes.insert(O, 1);
    f.calls.clear();
    object_event(&mut c, &t, &mut f, O, mevent::SPIKE).unwrap();
    assert_eq!(modes(&f), vec![0]);
    assert_eq!(c.get(O).unwrap().interact, 0);
    // Fissure in mode 1: nothing, no reschedule.
    let (mut c, mut f) = setup(FISSURE_CLASS, 1, 0, &mut t);
    object_event(&mut c, &t, &mut f, O, mevent::EVENT8).unwrap();
    assert!(f.calls.is_empty());
    let (mut c, mut f) = setup(FISSURE_CLASS, 0, 0, &mut t);
    object_event(&mut c, &t, &mut f, O, mevent::EVENT8).unwrap();
    assert_eq!(modes(&f), vec![1]);
    let mut e = Seed::new(1, 666);
    let lo = e.step();
    assert_eq!(
        f.schedules()[0],
        (O, mevent::EVENT8, 125 + (lo % 250) as i32)
    );
}

// Covers: specs/world/objects-2.md §17
#[test]
fn small_inits() {
    let mut t = tables();
    t.objects[CLASS as usize].mode2 = 1;
    for (n, mode, want_modes, want_sched) in [
        (8u8, 0u8, vec![2u8], vec![]),
        (14, 0, vec![1], vec![]),
        (26, 0, vec![1], vec![]),
        (22, 0, vec![2], vec![(O, mevent::FIRE, 125)]),
        (24, 0, vec![], vec![(O, mevent::SPIKE, 125)]),
        (51, 1, vec![], vec![(O, mevent::SOUL, 135)]),
        (51, 0, vec![], vec![]),
        (13, 0, vec![2], vec![]),
        (13, 2, vec![], vec![]),
    ] {
        let (mut c, mut f) = setup(CLASS, mode, 0, &mut t);
        init(&mut c, &t, &mut f, O, n, Some(ROOM), 5, 5).unwrap();
        assert_eq!(modes(&f), want_modes, "init {n}");
        assert_eq!(f.schedules(), want_sched, "init {n}");
    }
    // 27: lo' 1791398751 mod 1000 = 751 > 332 → 0.
    let (mut c, mut f) = setup(CLASS, 0, 0, &mut t);
    c.get_mut(O).unwrap().interact = 9;
    init(&mut c, &t, &mut f, O, 27, Some(ROOM), 5, 5).unwrap();
    assert_eq!(c.get(O).unwrap().interact, 0);
    // 34: lo' & 1 = 1 → mode 1.
    let (mut c, mut f) = setup(CLASS, 0, 0, &mut t);
    init(&mut c, &t, &mut f, O, 34, Some(ROOM), 5, 5).unwrap();
    assert_eq!(modes(&f), vec![1]);
    // 58: event 8 at f + 25 + 751 mod 250.
    let (mut c, mut f) = setup(CLASS, 0, 0, &mut t);
    init(&mut c, &t, &mut f, O, 58, Some(ROOM), 5, 5).unwrap();
    assert_eq!(f.schedules(), vec![(O, mevent::EVENT8, 125 + 1)]);
    // 28: mode 2, n = 1791398751 mod 9 + 1 = 7 rounds of 2 steps, no
    // room match (the fake finds none): no gold.
    let (mut c, mut f) = setup(CLASS, 0, 0, &mut t);
    init(&mut c, &t, &mut f, O, 28, Some(ROOM), 5, 5).unwrap();
    assert_eq!(modes(&f), vec![2]);
    let mut e = Seed::new(1, 666);
    assert_eq!(e.step() % 9 + 1, 7);
    for _ in 0..14 {
        e.step();
    }
    assert_eq!(c.seed, e);
}

// Covers: specs/world/objects-2.md §16.11
#[test]
fn stairs_open_and_stair2_gate() {
    let mut t = tables();
    let (mut c, mut f) = setup(CLASS, 0, 47, &mut t);
    assert_eq!(
        dispatch(&mut c, &t, &mut f, O, Some(P)),
        Ok(Dispatch::Done(1))
    );
    assert_eq!(modes(&f), vec![1]);
    let (mut c, mut f) = setup(CLASS, 0, 50, &mut t);
    assert_eq!(
        dispatch(&mut c, &t, &mut f, O, Some(P)),
        Ok(Dispatch::Done(0))
    );
    assert!(f.calls.is_empty());
}
