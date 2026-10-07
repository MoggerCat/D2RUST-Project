// Spec: specs/sim/intents-events.md §7.3, §7.4, §7.5, §7.7; specs/sim/units.md §4.1, §4.2, §4.6
//! The monster mode message in the client pass of the action wiring
//! with the path provider on: a kill sends 0x69 code 8 in the next
//! client pass, the end of the death animation sets mode 12 and sends
//! code 9, the room clean-up makes each one a single message.

use std::sync::Arc;

use d2_formats::animdata::{self, AnimData, AnimRecord};

use super::super::reaction::kill;
use super::super::tests::Fx;
use super::*;
use crate::units::lists::client_state;

const DEATH: &[u8; 8] = b"M0DTHTH\0";
/// Frames of the fixture's death animation (the recording's 24 frames
/// between code 8 and code 9, `intents-events.md` §7.7 rule 3).
const DEATH_FRAMES: u32 = 24;

fn anim_data() -> AnimData {
    let mut a = AnimData {
        buckets: vec![Vec::new(); animdata::BUCKETS],
    };
    let len = DEATH.iter().position(|&b| b == 0).unwrap();
    a.buckets[animdata::hash(&DEATH[..len])].push(AnimRecord {
        name: *DEATH,
        frames: DEATH_FRAMES,
        speed: 256,
        events: [0; animdata::EVENTS],
    });
    a
}

/// The fixture with the path provider on, a player with a client in
/// room A and a monster (class 0) next to it; one tick has run and the
/// messages it sent are dropped.
fn setup() -> (Fx, UnitId, UnitId) {
    let mut fx = Fx::new();
    let h = fx.sim.hooks();
    h.enable_paths().expect("embedded tables");
    h.anim_data = Some(Arc::new(anim_data()));
    h.x.names.insert((UnitType::Monster, 0), *DEATH);
    let a = fx.a;
    let p = fx.spawn(UnitType::Player, 1, a, 10, 10);
    let m = fx.spawn(UnitType::Monster, 0, a, 13, 10);
    // The client joins room A through the room switch of its first
    // per-client update (`rooms.md` §4.1), which fills the rooms' client
    // arrays (§7 rule 1).
    fx.game
        .lists
        .add_client(Some(p), None, client_state::IN_GAME);
    fx.tick();
    fx.sim.hooks().x.sent.clear();
    (fx, p, m)
}

fn kill_now(fx: &mut Fx, m: UnitId, p: UnitId) {
    let game = &mut fx.game;
    fx.sim.combat(game, |cv, _| kill(cv, m, p));
}

fn guid(fx: &Fx, u: UnitId) -> u32 {
    fx.game.lists.unit(u).unwrap().guid
}

fn path(fx: &mut Fx, u: UnitId) -> crate::path::DynamicPath {
    fx.sim
        .hooks()
        .paths
        .as_ref()
        .unwrap()
        .dynamic(u)
        .unwrap()
        .clone()
}

fn sent(fx: &mut Fx) -> Vec<(UnitId, Vec<u8>)> {
    std::mem::take(&mut fx.sim.hooks().x.sent)
}

/// §7.4 rule 7 / §7.7 rule 3: the kill sets mode 0 with flag 0x1, so the
/// next client pass sends 0x69 code 8 at the path target with d = the
/// path direction and e = unit +0xB0 (0 here: [`Pending::unit_b0`]'s
/// default); the clean-up then clears flag 0x1 and nothing more is sent
/// until event 1 of the DT animation (§4.2: f + 24 for 24 frames at
/// speed 256) sets mode 12, whose message (code 9, the unit's cell,
/// e = 0) goes out in that same tick.
// Covers: specs/sim/intents-events.md §7.4 r7, §7.7 r2, §7.7 r3; specs/sim/intents-events.md §7.3 r2
#[test]
fn kill_sends_code_8_then_the_death_end_code_9() {
    let (mut fx, p, m) = setup();
    {
        let d = fx
            .sim
            .hooks()
            .paths
            .as_mut()
            .unwrap()
            .dynamic_mut(m)
            .unwrap();
        d.direction = 0x38;
        d.target_x = 0x1295;
        d.target_y = 0x1555;
    }
    let f = fx.game.frame;
    kill_now(&mut fx, m, p);
    assert_eq!(fx.sim.sys.units.get(m).unwrap().mode, 0);
    let g = guid(&fx, m);
    fx.tick();
    let mut want = vec![0x69];
    want.extend(g.to_le_bytes());
    want.extend([0x08, 0x95, 0x12, 0x55, 0x15, 0x38, 0x00]);
    assert_eq!(sent(&mut fx), vec![(p, want)]);
    assert_eq!(
        fx.sim.sys.units.get(m).unwrap().flags & (flags::CHANGED | flags::MODE_CHANGING),
        0
    );
    // Event 1 at f + 24: nothing before it.
    let end = f + DEATH_FRAMES as i32;
    while fx.game.frame < end - 1 {
        fx.tick();
        assert_eq!(sent(&mut fx), vec![], "frame {}", fx.game.frame);
    }
    fx.tick();
    assert_eq!(fx.game.frame, end);
    assert_eq!(fx.sim.sys.units.get(m).unwrap().mode, 12);
    let d = path(&mut fx, m);
    let mut want = vec![0x69];
    want.extend(g.to_le_bytes());
    want.push(0x09);
    want.extend((d.x() as u16).to_le_bytes());
    want.extend((d.y() as u16).to_le_bytes());
    want.extend([0x38, 0x00]);
    assert_eq!(sent(&mut fx), vec![(p, want)]);
    for _ in 0..30 {
        fx.tick();
        assert_eq!(sent(&mut fx), vec![]);
    }
    fx.assert_clean();
}

/// Event 1 of DT runs unit event 13 (`0x005C0C30(game, 13, unit, 0, 0)`)
/// after the mode set; `SplEndDeath` 0 runs no death action.
// Covers: specs/sim/intents-events.md §7.7 r3
#[test]
fn death_end_runs_unit_event_13() {
    let (mut fx, p, m) = setup();
    kill_now(&mut fx, m, p);
    for _ in 0..DEATH_FRAMES {
        fx.tick();
    }
    let log = &fx.sim.hooks().x.log;
    let ev = format!("event 13 Some({})", m.0);
    assert_eq!(log.iter().filter(|l| **l == ev).count(), 1, "{log:?}");
    assert!(
        !log.iter().any(|l| l.starts_with("death action")),
        "{log:?}"
    );
}

/// A monster of base id 78 steps and sets mode 12 only when its
/// animation is complete (`0x006217C0`); every other monster at once.
// Covers: specs/sim/intents-events.md §7.7 r3
#[test]
fn anim_complete_reads_frame_speed_and_count() {
    let mut a = Anim {
        frame: 0x500,
        frame_count: 0x600,
        speed: 0x100,
        ..Anim::default()
    };
    assert!(anim_complete(&a));
    a.speed = 0xFF;
    assert!(!anim_complete(&a));
    a.sequence = Some(crate::units::record::Sequence {
        frame_count: 0,
        speed: 0,
        events: Vec::new(),
    });
    a.frame_count = 0;
    assert!(anim_complete(&a));
    a.frame_count = 1;
    assert!(!anim_complete(&a));
}

/// §7.5 step 3 and the flag bits of step 7, per type.
// Covers: specs/sim/intents-events.md §7.5 r3
#[test]
fn room_cleanup_clears_the_listed_flags() {
    let (mut fx, p, m) = setup();
    for u in [p, m] {
        let r = fx.sim.sys.units.get_mut(u).unwrap();
        r.flags = u32::MAX;
        r.flags2 = u32::MAX;
    }
    if let Some(d) = fx.sim.hooks().paths.as_mut().unwrap().dynamic_mut(m) {
        d.flags = u32::MAX;
    }
    let s = &mut fx.sim.sys;
    let mut v = View::of(&mut s.units, &mut s.stats, &s.data, &mut s.hooks);
    v.room_cleanup(p);
    v.room_cleanup(m);
    let rp = fx.sim.sys.units.get(p).unwrap();
    assert_eq!(rp.flags, !(0x1 | 0x10 | 0x400 | 0x8000 | 0x100));
    assert_eq!(rp.flags2, !(0x800 | 0x1000 | 0x10000 | 0x200000));
    let rm = fx.sim.sys.units.get(m).unwrap();
    assert_eq!(rm.flags, !(0x1 | 0x10 | 0x400 | 0x8000 | 0x100 | 0x800));
    assert_eq!(rm.flags2, !(0x800 | 0x1000 | 0x10000 | 0x200000));
    assert_eq!(path(&mut fx, m).flags, !0x2);
}

/// §7.5 steps 4 and 7: the state-changed bits are zeroed and a player's
/// stat 29 `lastexp` := −1.
// Covers: specs/sim/intents-events.md §7.5 r4, §7.5 r7
#[test]
fn room_cleanup_resets_changed_states_and_lastexp() {
    let (mut fx, p, _) = setup();
    fx.sim.sys.stats.set_state_changed(p, 1, true);
    let s = &mut fx.sim.sys;
    let mut v = View::of(&mut s.units, &mut s.stats, &s.data, &mut s.hooks);
    v.room_cleanup(p);
    let (_, changed) = fx.sim.sys.stats.state_bits(p).expect("state bits");
    assert!(changed.iter().all(|&w| w == 0), "{changed:?}");
    assert_eq!(fx.sim.sys.stats.unit_base(p, 29, 0), -1);
}

/// §7.4 rule 4: with the provider on and no path record, the mode
/// message is the fatal 0xE6 (logged, nothing sent).
// Covers: specs/sim/intents-events.md §7.4 r4
#[test]
fn a_monster_without_a_path_is_the_fatal() {
    let (mut fx, p, m) = setup();
    fx.sim.hooks().paths.as_mut().unwrap().records.remove(&m);
    fx.sim.sys.units.get_mut(m).unwrap().flags |= flags::CHANGED;
    fx.game.lists.queue_update(m).unwrap();
    fx.tick();
    assert_eq!(sent(&mut fx), vec![]);
    assert_eq!(
        fx.sim.hooks().errors,
        vec![WiringError::ModeMessage(ModeMessageError::NoPath(m))]
    );
    let _ = p;
}

/// §7.4 rule 2: a path target unit that is gone is cleared by the
/// refresh and gives no target (0x69 code 6 of mode 3 to the cell, not
/// 0x6A); one in a room the client is in is kept (0x6A for mode 6).
// Covers: specs/sim/intents-events.md §7.4 r2
#[test]
fn the_target_is_refreshed_and_tested_against_the_client_rooms() {
    let (mut fx, p, m) = setup();
    let gp = guid(&fx, p);
    let gm = guid(&fx, m);
    let set_target = |fx: &mut Fx, unit: UnitId, guid: u32| {
        let d = fx
            .sim
            .hooks()
            .paths
            .as_mut()
            .unwrap()
            .dynamic_mut(m)
            .unwrap();
        d.target_unit = Some(crate::path::TargetUnit {
            unit,
            ty: UnitType::Player,
            guid,
        });
    };
    // Mode 6 with the player as target: the player's room holds the client.
    set_target(&mut fx, p, gp);
    fx.sim.sys.units.get_mut(m).unwrap().mode = 6;
    fx.sim.sys.units.get_mut(m).unwrap().flags |= flags::CHANGED;
    fx.game.lists.queue_update(m).unwrap();
    fx.tick();
    let mut want = vec![0x6A];
    want.extend(gm.to_le_bytes());
    want.extend([18, 0]);
    want.extend(gp.to_le_bytes());
    want.push(0);
    assert_eq!(sent(&mut fx), vec![(p, want)]);
    // A stale GUID: cleared, mode 6 without a target sends (0, −1).
    set_target(&mut fx, p, gp + 100);
    fx.sim.sys.units.get_mut(m).unwrap().flags |= flags::CHANGED;
    fx.game.lists.queue_update(m).unwrap();
    fx.tick();
    let mut want = vec![0x69];
    want.extend(gm.to_le_bytes());
    want.extend([18, 0, 0, 0xFF, 0xFF, 0, 0]);
    assert_eq!(sent(&mut fx), vec![(p, want)]);
    assert_eq!(path(&mut fx, m).target_unit, None);
    fx.assert_clean();
}

/// §7.3 rule 2 step 1: flag-ex 0x10000 sends S→C 0x15 (type 1, GUID,
/// the path's cell, flag 1), then the room-change messages (`pathing.md`
/// §9.8: nothing without path flag 0x2); step 4: unit flag 0x100 with no
/// hover text sends 0x76 (§7.9 rule 3). Step 1 comes first. The room
/// clean-up clears both bits (§7.5), so the next pass sends nothing.
// Covers: specs/sim/intents-events.md §7.3 r2, §7.9 r3, §7.5 r3, §7.5 r7
#[test]
fn reassign_and_overhead_steps_of_the_monster_update() {
    let (mut fx, p, m) = setup();
    let g = guid(&fx, m).to_le_bytes();
    {
        let r = fx.sim.sys.units.get_mut(m).unwrap();
        r.flags |= flags::HOVER_FREED;
        r.flags2 |= 0x1_0000;
    }
    fx.game.lists.queue_update(m).unwrap();
    let d = path(&mut fx, m);
    fx.tick();
    let mut reassign = vec![0x15, 1, g[0], g[1], g[2], g[3]];
    reassign.extend((d.x() as u16).to_le_bytes());
    reassign.extend((d.y() as u16).to_le_bytes());
    reassign.push(1);
    let overhead = vec![0x76, 1, g[0], g[1], g[2], g[3]];
    assert_eq!(sent(&mut fx), vec![(p, reassign), (p, overhead)]);
    fx.game.lists.queue_update(m).unwrap();
    fx.tick();
    assert_eq!(sent(&mut fx), vec![]);
    fx.assert_clean();
}

/// Perturbation of the test above: the same bits on a monster that is
/// not queued for update send nothing (the client pass walks the update
/// queues, §7.1 rule 1).
// Covers: specs/sim/intents-events.md §7.1 r1
#[test]
fn unqueued_monster_bits_send_nothing() {
    let (mut fx, _, m) = setup();
    {
        let r = fx.sim.sys.units.get_mut(m).unwrap();
        r.flags |= flags::HOVER_FREED;
        r.flags2 |= 0x1_0000;
    }
    fx.tick();
    assert_eq!(sent(&mut fx), vec![]);
}
