// Spec: specs/world/objects.md §10–§13 (Test vectors, Edge cases)
use super::super::fake::{blank_object, Call, Fake};
use super::super::{dispatch, oevent, oflags, Dispatch, ObjectData};
use super::*;
use crate::rng::Seed;

// Test conventions for the stat map of [`Fake`] (it has no fields for the
// misc seams): keys at or above 0xF000 are not stats.
/// OR of the collision bits of the object's footprint cells.
const CELLS: u16 = 0xFFFF;
/// `iterate_players` result.
const ITERATE: u16 = 0xFFFE;
/// `hostile_time` value.
const HOSTILE: u16 = 0xFFFD;
/// Nonzero: `cure_states` removes something.
const CURABLE: u16 = 0xFFFC;
/// Nonzero: a pet is healed.
const PETS: u16 = 0xFFFB;
/// Nonzero: `portal_travel` runs and returns this value.
const TRAVEL: u16 = 0xFFFA;
/// State `s` is present when key 0xF000 + s is nonzero.
const STATE: u16 = 0xF000;

impl MiscWorld for Fake {
    fn create_level_portal(&mut self, object: UnitId, row: u16) {
        self.calls
            .push(Call::Other(format!("level_portal {object:?} {row}")));
    }
    fn footprint_collides(&self, object: UnitId, mask: u16) -> bool {
        let cells = self.stats.get(&(object, CELLS)).copied().unwrap_or(0) as u16;
        cells & mask != 0
    }
    fn vital_stat(&self, unit: UnitId, id: u16) -> u32 {
        self.stats.get(&(unit, id)).copied().unwrap_or(0) as u32
    }
    fn set_vital_stat(&mut self, unit: UnitId, id: u16, value: u32) {
        self.stats.insert((unit, id), value as i32);
        self.calls.push(Call::Other(format!("stat {id} {value}")));
    }
    fn remove_state_list(&mut self, unit: UnitId, state: u16) -> bool {
        self.calls
            .push(Call::Other(format!("remove state {state}")));
        self.stats.remove(&(unit, STATE + state)).unwrap_or(0) != 0
    }
    fn cure_states(&mut self, unit: UnitId) -> bool {
        self.calls.push(Call::Other("cure".into()));
        self.stats.remove(&(unit, CURABLE)).unwrap_or(0) != 0
    }
    fn well_heal_pets(&mut self, player: UnitId, _object: UnitId) -> bool {
        self.calls.push(Call::Other("pets".into()));
        self.stats.get(&(player, PETS)).copied().unwrap_or(0) != 0
    }
    fn iterate_players(&self, player: UnitId) -> u32 {
        self.stats.get(&(player, ITERATE)).copied().unwrap_or(0) as u32
    }
    fn hostile_time(&self, player: UnitId) -> u32 {
        self.stats.get(&(player, HOSTILE)).copied().unwrap_or(0) as u32
    }
    fn portal_travel(&mut self, object: UnitId, player: UnitId) -> Option<i32> {
        self.calls
            .push(Call::Other(format!("travel {object:?} {player:?}")));
        self.stats.get(&(player, TRAVEL)).copied()
    }
}

const O: UnitId = UnitId(10);
const P: UnitId = UnitId(20);
const M: UnitId = UnitId(30);

const DOOR: u16 = 1;
const WELL: u16 = 2;
const PORTAL: u16 = 3;
const TORCH: u16 = 4;

fn tables() -> ObjectTables {
    let mut objects: Vec<_> = (0..8).map(|_| blank_object()).collect();
    for (c, f) in [(DOOR, 8), (WELL, 22), (PORTAL, 15), (TORCH, 11)] {
        objects[c as usize].operatefn = f;
        // Sync 1: mode changes draw nothing (§4).
        objects[c as usize].sync = 1;
    }
    // Live well values (§11): Parm0 750, Parm1 128, Parm2 1, Parm3 3.
    let w = &mut objects[WELL as usize];
    (w.parm0, w.parm1, w.parm2, w.parm3) = (750, 128, 1, 3);
    ObjectTables {
        objects,
        ..ObjectTables::default()
    }
}

/// Object `O` of `class` in `mode`, player `P`, monster `M`.
fn setup(class: u16, mode: u8) -> (ObjectControl, Fake) {
    let mut ctl = ObjectControl {
        seed: Seed::init(),
        regions: vec![],
        shrine_lists: Default::default(),
        data: Default::default(),
    };
    ctl.data.insert(
        O,
        ObjectData {
            guid: 0x55,
            class,
            ..ObjectData::default()
        },
    );
    let mut f = Fake::default();
    f.modes.insert(O, mode);
    f.guids.insert(O, 0x55);
    f.guids.insert(P, 0x77);
    f.operators.insert(P, Operator::Player(0));
    f.operators.insert(M, Operator::Monster);
    f.tick = 100_000;
    (ctl, f)
}

fn run(ctl: &mut ObjectControl, t: &ObjectTables, f: &mut Fake, op: Option<UnitId>) -> Dispatch {
    dispatch(ctl, t, f, O, op).unwrap()
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

// ------------------------------------------------------------------ §10

// Covers: specs/world/objects.md §10 text, §10 r1, §10 r3, §edge-cases-original-bugs r9
#[test]
fn door_debounce_host_tick() {
    let t = tables();
    let (mut ctl, mut f) = setup(DOOR, 0);
    ctl.get_mut(O).unwrap().last_tick = 1000;
    f.tick = 1499;
    assert_eq!(run(&mut ctl, &t, &mut f, Some(P)), Dispatch::Done(1));
    assert!(f.calls.is_empty());
    f.tick = 1500;
    assert_eq!(run(&mut ctl, &t, &mut f, Some(P)), Dispatch::Done(1));
    assert_eq!(f.calls[..2], [Call::Free(O), Call::Mode(O, 2, true)]);
    assert_eq!(ctl.get(O).unwrap().last_tick, 1500);
}

// Covers: specs/world/objects.md §10 r2
#[test]
fn door_open_and_locked() {
    let t = tables();
    // Closed: free the footprint, mode 2, tick stored.
    let (mut ctl, mut f) = setup(DOOR, 0);
    run(&mut ctl, &t, &mut f, Some(P));
    assert_eq!(f.modes[&O], 2);
    assert_eq!(ctl.get(O).unwrap().last_tick, 100_000);
    // Locked without a key: sound 22 on the operator, nothing else.
    let (mut ctl, mut f) = setup(DOOR, 6);
    assert_eq!(run(&mut ctl, &t, &mut f, Some(P)), Dispatch::Done(1));
    assert_eq!(
        f.calls,
        vec![Call::KeyTest(P), Call::Sound(P, sound::LOCKED, None, false)]
    );
    assert_eq!(ctl.get(O).unwrap().last_tick, 0);
    // With a key: as mode 0.
    f.keys.insert(P);
    f.calls.clear();
    run(&mut ctl, &t, &mut f, Some(P));
    assert_eq!(
        f.calls[..3],
        [Call::KeyTest(P), Call::Free(O), Call::Mode(O, 2, true)]
    );
    assert_eq!(ctl.get(O).unwrap().last_tick, 100_000);
    // Modes 1, 3, 4, 7: nothing.
    for m in [1, 3, 4, 7] {
        let (mut ctl, mut f) = setup(DOOR, m);
        assert_eq!(run(&mut ctl, &t, &mut f, Some(P)), Dispatch::Done(1));
        assert!(f.calls.is_empty());
        assert_eq!(ctl.get(O).unwrap().last_tick, 0);
    }
}

// Covers: specs/world/objects.md §10 r2, §edge-cases-original-bugs r8
#[test]
fn door_close_blocked() {
    let t = tables();
    // Clear doorway: stamp, mode 0.
    let (mut ctl, mut f) = setup(DOOR, 2);
    run(&mut ctl, &t, &mut f, Some(P));
    assert_eq!(f.calls[..2], [Call::Stamp(O), Call::Mode(O, 0, true)]);
    assert_eq!(ctl.get(O).unwrap().last_tick, 100_000);
    // Vector: mode 2, a player (0x80) only → mode 5, tick stored.
    let (mut ctl, mut f) = setup(DOOR, 2);
    f.stats.insert((O, CELLS), 0x80);
    run(&mut ctl, &t, &mut f, Some(P));
    assert_eq!(modes(&f), vec![5]);
    assert!(!f.calls.contains(&Call::Stamp(O)));
    assert_eq!(ctl.get(O).unwrap().last_tick, 100_000);
    // Already 5 with a monster: nothing, no tick.
    let (mut ctl, mut f) = setup(DOOR, 5);
    f.stats.insert((O, CELLS), 0x100);
    run(&mut ctl, &t, &mut f, Some(P));
    assert!(f.calls.is_empty());
    assert_eq!(ctl.get(O).unwrap().last_tick, 0);
    // A corpse (0x8000) → mode 4, from 2 or 5.
    for m in [2, 5] {
        let (mut ctl, mut f) = setup(DOOR, m);
        f.stats.insert((O, CELLS), 0x8080);
        run(&mut ctl, &t, &mut f, Some(P));
        assert_eq!(modes(&f), vec![4]);
        assert_eq!(ctl.get(O).unwrap().last_tick, 100_000);
    }
    // Mode 5, doorway clear: closes.
    let (mut ctl, mut f) = setup(DOOR, 5);
    f.stats.insert((O, CELLS), 0x7F);
    run(&mut ctl, &t, &mut f, Some(P));
    assert_eq!(modes(&f), vec![0]);
}

// ------------------------------------------------------------------ §11

fn well_fake(charges: u8) -> (ObjectControl, Fake) {
    let (mut ctl, mut f) = setup(WELL, 0);
    ctl.get_mut(O).unwrap().interact = charges;
    f.frame = 1000;
    (ctl, f)
}

/// Life 100/200, mana 50/100, stamina full (8.8 fixed).
fn hurt(f: &mut Fake) {
    for (id, v) in [(6, 100), (7, 200), (8, 50), (9, 100), (10, 30), (11, 30)] {
        f.stats.insert((P, id), v << 8);
    }
}

// Covers: specs/world/objects.md §11 text, §11 r1, §11 r2, §11 r3, §edge-cases-original-bugs r10
#[test]
fn well_heals_and_uses_a_charge() {
    let t = tables();
    let (mut ctl, mut f) = well_fake(2);
    hurt(&mut f);
    assert_eq!(run(&mut ctl, &t, &mut f, Some(P)), Dispatch::Done(0));
    // life + 200·128 >> 8 = +100 → 200 (max); mana + 50 → 100.
    assert_eq!(f.stats[&(P, 6)], 200 << 8);
    assert_eq!(f.stats[&(P, 8)], 100 << 8);
    assert_eq!(
        f.calls[..6],
        [
            Call::Other(format!("stat 6 {}", 200 << 8)),
            Call::Other(format!("stat 8 {}", 100 << 8)),
            Call::Other("remove state 2".into()),
            Call::Other("remove state 1".into()),
            Call::Other("cure".into()),
            Call::Other("pets".into()),
        ]
    );
    assert_eq!(ctl.get(O).unwrap().interact, 1);
    assert_eq!(modes(&f), vec![1]);
    assert_eq!(f.schedules(), vec![(O, oevent::WELL_REFILL, 1751)]);
    // Partial heal: min(life + max·Parm1 >> 8, max).
    let (mut ctl, mut f) = well_fake(2);
    hurt(&mut f);
    f.stats.insert((P, 6), 10 << 8);
    run(&mut ctl, &t, &mut f, Some(P));
    assert_eq!(f.stats[&(P, 6)], 110 << 8);
}

// Covers: specs/world/objects.md §11 r1, §11 r2, §11 r3
#[test]
fn well_parm3_and_unused() {
    let mut t = tables();
    // Charges 0 → 0, nothing.
    let (mut ctl, mut f) = well_fake(0);
    hurt(&mut f);
    assert_eq!(run(&mut ctl, &t, &mut f, Some(P)), Dispatch::Done(0));
    assert!(f.calls.is_empty());
    // Full player: nothing used, the charge stays.
    let (mut ctl, mut f) = well_fake(2);
    run(&mut ctl, &t, &mut f, Some(P));
    assert_eq!(ctl.get(O).unwrap().interact, 2);
    assert!(f.schedules().is_empty());
    assert!(modes(&f).is_empty());
    // Parm3 = 2: life only; stamina always.
    t.objects[WELL as usize].parm3 = 2;
    let (mut ctl, mut f) = well_fake(2);
    hurt(&mut f);
    f.stats.insert((P, 10), 0);
    run(&mut ctl, &t, &mut f, Some(P));
    assert_eq!(f.stats[&(P, 6)], 200 << 8);
    assert_eq!(f.stats[&(P, 8)], 50 << 8);
    assert_eq!(f.stats[&(P, 10)], 15 << 8);
    // Parm3 = 0, only a poison state removed: still used.
    t.objects[WELL as usize].parm3 = 0;
    let (mut ctl, mut f) = well_fake(2);
    hurt(&mut f);
    f.stats.insert((P, STATE + 2), 1);
    run(&mut ctl, &t, &mut f, Some(P));
    assert_eq!(f.stats[&(P, 6)], 100 << 8);
    assert_eq!(ctl.get(O).unwrap().interact, 1);
    // Curable states, pets: used.
    for key in [CURABLE, PETS] {
        let (mut ctl, mut f) = well_fake(2);
        f.stats.insert((P, key), 1);
        run(&mut ctl, &t, &mut f, Some(P));
        assert_eq!(ctl.get(O).unwrap().interact, 1);
    }
}

// Covers: specs/world/objects.md §11 r3, §11 text
#[test]
fn well_charge_vector() {
    // Parm2 1: charges 2, use, use, refill, refill → modes 1, 2, 1, 0.
    let t = tables();
    let (mut ctl, mut f) = well_fake(2);
    hurt(&mut f);
    run(&mut ctl, &t, &mut f, Some(P));
    hurt(&mut f);
    run(&mut ctl, &t, &mut f, Some(P));
    assert_eq!(ctl.get(O).unwrap().interact, 0);
    // Empty: no heal.
    hurt(&mut f);
    run(&mut ctl, &t, &mut f, Some(P));
    assert_eq!(f.stats[&(P, 6)], 100 << 8);
    well_refill(&mut ctl, &t, &mut f, O).unwrap();
    assert_eq!(ctl.get(O).unwrap().interact, 1);
    well_refill(&mut ctl, &t, &mut f, O).unwrap();
    assert_eq!(ctl.get(O).unwrap().interact, 2);
    assert_eq!(modes(&f), vec![1, 2, 1, 0]);
    assert_eq!(f.modes[&O], 0);
    // Refill: the ordinary mode set (queues), then queue and flag 0x1
    // again (`objects-2.md` §24 rule 7).
    let n = f.calls.len();
    assert_eq!(
        f.calls[n - 3..],
        [
            Call::Mode(O, 0, true),
            Call::Anim(O, 0, 0, 0),
            Call::Queue(O)
        ]
    );
    assert_ne!(f.flags[&O] & oflags::CHANGED, 0);
    // A third refill is fatal (c / Parm2 ≥ 2).
    assert_eq!(
        well_refill(&mut ctl, &t, &mut f, O),
        Err(ObjectError::WellCharges(2))
    );
}

// Covers: specs/world/objects.md §11 r3
#[test]
fn well_larger_parm2_modes() {
    // Parm2 2 (4 charges): mode changes only at multiples of 2.
    let mut t = tables();
    t.objects[WELL as usize].parm2 = 2;
    let (mut ctl, mut f) = well_fake(4);
    for _ in 0..4 {
        hurt(&mut f);
        run(&mut ctl, &t, &mut f, Some(P));
    }
    assert_eq!(modes(&f), vec![1, 2]);
    for _ in 0..4 {
        well_refill(&mut ctl, &t, &mut f, O).unwrap();
    }
    assert_eq!(modes(&f), vec![1, 2, 1, 0]);
    assert_eq!(f.calls.iter().filter(|c| **c == Call::Queue(O)).count(), 4);
    // c / Parm2 = 2 → fatal.
    assert!(well_refill(&mut ctl, &t, &mut f, O).is_err());
}

// ------------------------------------------------------------------ §12

// Covers: specs/world/objects.md §12 text, §12 r1
#[test]
fn portal_busy_and_owner() {
    let t = tables();
    for which in 0..3 {
        let (mut ctl, mut f) = setup(PORTAL, 1);
        // The dispatch refuses these first (§7.2 rule 2); the portal's own
        // busy test is reached by a direct call.
        match which {
            0 => f.interact_active.insert(P),
            1 => f.busy.insert(P),
            _ => f.cursor.insert(P),
        };
        let op = Operate {
            object: O,
            operator: Some(P),
            class: PORTAL,
            operate_fn: 15,
        };
        assert_eq!(portal(&mut ctl, &t, &mut f, &op), Ok(Some(0)));
        assert!(f.calls.is_empty());
    }
    // Iterate-players nonzero: only the owner passes.
    let (mut ctl, mut f) = setup(PORTAL, 1);
    f.stats.insert((P, ITERATE), 1);
    ctl.get_mut(O).unwrap().owner = Some(-1);
    assert_eq!(run(&mut ctl, &t, &mut f, Some(P)), Dispatch::Done(0));
    assert!(f.calls.is_empty());
    ctl.get_mut(O).unwrap().owner = Some(0x77);
    f.stats.insert((P, TRAVEL), 1);
    assert_eq!(run(&mut ctl, &t, &mut f, Some(P)), Dispatch::Done(1));
    // Iterate-players 0: anyone.
    let (mut ctl, mut f) = setup(PORTAL, 1);
    f.stats.insert((P, TRAVEL), 1);
    assert_eq!(run(&mut ctl, &t, &mut f, Some(P)), Dispatch::Done(1));
    // §12 rule 5: a monster or no operator is fatal (`0x0058494F`; §7.1
    // stops monsters before the dispatch with live data).
    for op in [Some(M), None] {
        assert_eq!(
            dispatch(&mut ctl, &t, &mut f, O, op),
            Err(ObjectError::PortalOperator)
        );
    }
}

// Covers: specs/world/objects.md §12 r2, §12 r3, §edge-cases-original-bugs r9
#[test]
fn portal_hostile_delay_and_travel_seam() {
    let t = tables();
    let (mut ctl, mut f) = setup(PORTAL, 1);
    f.stats.insert((P, HOSTILE), 10_000);
    f.tick = 14_999;
    assert_eq!(run(&mut ctl, &t, &mut f, Some(P)), Dispatch::Done(0));
    assert_eq!(
        f.calls,
        vec![Call::Sound(P, sound::PORTAL_REFUSED, None, false)]
    );
    // 5000 ms later: rule 3, which the default host does not run.
    f.tick = 15_000;
    f.calls.clear();
    let op = Operate {
        object: O,
        operator: Some(P),
        class: PORTAL,
        operate_fn: 15,
    };
    assert_eq!(run(&mut ctl, &t, &mut f, Some(P)), Dispatch::NotCovered(op));
    assert_eq!(f.calls, vec![Call::Other(format!("travel {O:?} {P:?}"))]);
}

// ------------------------------------------------------------------ §13

// Covers: specs/world/objects.md §13
#[test]
fn torch_modes() {
    let t = tables();
    let (mut ctl, mut f) = setup(TORCH, 0);
    f.flags.insert(O, oflags::SELECTABLE);
    assert_eq!(run(&mut ctl, &t, &mut f, Some(P)), Dispatch::Done(1));
    assert_eq!(f.modes[&O], 1);
    assert_eq!(f.flags[&O] & oflags::SELECTABLE, 0);
    for m in [1, 2] {
        let (mut ctl, mut f) = setup(TORCH, m);
        f.flags.insert(O, oflags::SELECTABLE);
        assert_eq!(run(&mut ctl, &t, &mut f, Some(P)), Dispatch::Done(1));
        assert_eq!(f.modes[&O], 0);
        assert_ne!(f.flags[&O] & oflags::SELECTABLE, 0);
    }
    for m in [3, 7] {
        let (mut ctl, mut f) = setup(TORCH, m);
        assert_eq!(run(&mut ctl, &t, &mut f, Some(P)), Dispatch::Done(1));
        assert!(f.calls.is_empty());
    }
}
