// Spec: specs/world/objects-client.md (Test vectors), specs/client/model.md (§5 rules 2–3, §8 rules 4 and 7)
//! The spec's synthetic vectors: seed {1, 666} for U (or P), so lo' =
//! 1,791,398,751 and lo'' = 791,599,131.

use super::super::dispatch::{Dispatch, HandlerError};
use super::super::output::Output;
use super::super::receive::ReceiveLog;
use super::super::skills::{SkillEntry, SkillList};
use super::super::update::update_pass;
use super::super::world::{
    ActLoad, ClientUnit, ClientWorld, ModelInputs, UnitKey, INIT_SEED, MONSTER, OBJECT, PLAYER,
};
use super::fns::{CHICKEN, KEEPER_SOUND, OVERLAY_KIND_LOOP, OVERLAY_NPCALERT};
use super::interact::{self, interact_bytes};
use super::*;
use crate::rules::lighting::environment::Environment;

const LO1: u32 = 1_791_398_751;
const LO2: u32 = 791_599_131;
const OBJ: UnitKey = UnitKey::new(OBJECT, 5);
const P: UnitKey = UnitKey::new(PLAYER, 1);

/// A row of class 0 with `ClientFn` f; `FrameCnt` 21 (× 256) for every
/// mode, speed 0, nothing cycles.
fn row(f: u8) -> ObjClientRow {
    ObjClientRow {
        client_fn: f,
        frame_cnt: [21 * 256; 8],
        ..ObjClientRow::default()
    }
}

fn inputs(f: u8, now: u32) -> ModelInputs {
    let mut i = ModelInputs::default();
    i.objclient.rows = vec![row(f)];
    i.now = now;
    i
}

/// One S object (class 0) at (100, 100), seed {1, 666}, and the local
/// player at `p_at`.
fn world(p_at: (u16, u16)) -> ClientWorld {
    let mut w = ClientWorld::default();
    let mut o = ClientUnit::new(OBJ);
    o.position = Some((100, 100));
    w.units.insert(OBJ, o);
    let mut p = ClientUnit::new(P);
    p.position = Some(p_at);
    w.units.insert(P, p);
    w.local_player = Some(P);
    w
}

/// Calls U's `ClientFn` (the dispatch alone).
fn call(
    w: &mut ClientWorld,
    i: &ModelInputs,
    unit: ObjUnit,
) -> (Result<bool, HandlerError>, Vec<Output>) {
    let mut out = Vec::new();
    let row = i.objclient.rows[unit_ref(w, unit).unwrap().class as usize];
    let mut cx = Cx {
        w,
        inputs: i,
        unit,
        row,
        out: &mut out,
    };
    let r = dispatch(&mut cx);
    (r, out)
}

const S: ObjUnit = ObjUnit {
    key: OBJ,
    client_only: false,
};

fn obj(w: &ClientWorld) -> &ClientUnit {
    &w.units[&OBJ]
}

fn obj_mut(w: &mut ClientWorld) -> &mut ClientUnit {
    w.units.get_mut(&OBJ).unwrap()
}

fn mode_sounds(out: &[Output]) -> usize {
    out.iter()
        .filter(|o| matches!(o, Output::ObjectSound(ObjSound::Mode { .. })))
        .count()
}

// Covers: specs/world/objects-client.md §25 r1, §25 r2, §28 r2
#[test]
fn dispatch_zero_and_out_of_range() {
    // ClientFn 0: returns 1, the mode sound call at site A.
    let mut w = world((0, 0));
    let i = inputs(0, 0);
    assert!(call(&mut w, &i, S).0.unwrap());
    let mut out = Vec::new();
    object_update(&mut w, &i, S, &mut out).unwrap();
    assert_eq!(
        out,
        [Output::ObjectSound(ObjSound::Mode {
            unit: S,
            class: 0,
            mode: 0
        })]
    );
    // ClientFn 19 in a modded row: fatal 0x546.
    let i = inputs(19, 0);
    assert!(matches!(
        call(&mut w, &i, S).0,
        Err(HandlerError::Fatal(0x546))
    ));
    assert!(matches!(
        object_update(&mut w, &i, S, &mut Vec::new()),
        Err(HandlerError::Fatal(0x546))
    ));
    // No rows (the headless configuration): nothing runs.
    let mut out = Vec::new();
    object_update(&mut w, &ModelInputs::default(), S, &mut out).unwrap();
    assert!(out.is_empty());
}

// Covers: specs/world/objects-client.md §25 r2, §25 r3, §26.1, §26.11, §edge-cases-original-bugs r1
#[test]
fn call_sites_a_and_b() {
    // S object, ClientFn 2: never called (site A only sounds).
    let mut w = world((0, 0));
    let i = inputs(2, 0);
    let mut out = Vec::new();
    object_update(&mut w, &i, S, &mut out).unwrap();
    assert_eq!(obj(&w).flag_ex, 0);
    assert_eq!(obj(&w).interact_ms, 0);
    assert_eq!(mode_sounds(&out), 1);
    // ClientFn 1 and 11 return 1 and change nothing.
    for f in [1, 11] {
        let i = inputs(f, 0);
        let before = w.clone();
        assert!(call(&mut w, &i, S).0.unwrap());
        assert_eq!(w, before);
    }
    // A C object with ClientFn 4 runs it twice per update (A then B):
    // the drinker's own sound call twice, no sound from site A.
    let mut w = world((0, 0));
    let c = create_client_unit(&mut w, OBJECT, 0, 0, 0).unwrap();
    let i = inputs(4, 100);
    let mut out = Vec::new();
    let mut log = ReceiveLog::default();
    w.in_game = true;
    w.units.remove(&OBJ);
    update_pass(
        &mut w,
        &i,
        &Dispatch::from_spec().unwrap(),
        &mut log,
        &mut out,
    );
    assert!(log.rejected.is_empty(), "{:?}", log.rejected);
    assert_eq!(mode_sounds(&out), 2);
    // The first call drew the timer; the second found T ≥ now.
    assert_eq!(w.objclient.set_c[&c].interact_ms, 100 + 7_000 + LO1 % 5_000);
}

// Covers: specs/client/model.md §5 r3, §5 r4; specs/world/objects-client.md §28 r2
#[test]
fn update_order_c_objects_before_s_players() {
    let mut w = world((0, 0));
    w.in_game = true;
    let c = create_client_unit(&mut w, OBJECT, 0, 0, 0).unwrap();
    let mut out = Vec::new();
    let mut log = ReceiveLog::default();
    update_pass(
        &mut w,
        &inputs(0, 0),
        &Dispatch::from_spec().unwrap(),
        &mut log,
        &mut out,
    );
    let units: Vec<ObjUnit> = out
        .iter()
        .filter_map(|o| match o {
            Output::ObjectSound(ObjSound::Mode { unit, .. }) => Some(*unit),
            _ => None,
        })
        .collect();
    assert_eq!(
        units,
        [
            ObjUnit {
                key: c,
                client_only: true
            },
            S
        ]
    );
}

// Covers: specs/world/objects-client.md §26.7, §25 r5
#[test]
fn clientfn_7_vectors() {
    let mut w = world((0, 0));
    // Mode 0, T 0, now 5,000 → T 15,000, mode 1.
    let i = inputs(7, 5_000);
    let (r, out) = call(&mut w, &i, S);
    assert!(r.unwrap());
    assert_eq!((obj(&w).interact_ms, obj(&w).mode), (15_000, 1));
    assert_eq!(
        out,
        [Output::ObjectFx(ObjFx::GfxRefresh { unit: S, mode: 1 })]
    );
    // Mode 2, T 15,000, now 15,001 → 4,500 + 2,751 → T 22,252, mode 0.
    obj_mut(&mut w).mode = 2;
    let i = inputs(7, 15_001);
    call(&mut w, &i, S).0.unwrap();
    assert_eq!(LO1 % 3_000, 2_751);
    assert_eq!((obj(&w).interact_ms, obj(&w).mode), (22_252, 0));
    assert_eq!(obj(&w).seed, Some((LO1, 0)));
    // T not yet passed: nothing.
    let before = w.clone();
    call(&mut w, &inputs(7, 22_252), S).0.unwrap();
    assert_eq!(w, before);
}

// Covers: specs/world/objects-client.md §26.10
#[test]
fn clientfn_10_vector() {
    let mut w = world((0, 0));
    obj_mut(&mut w).mode = 1;
    call(&mut w, &inputs(10, 1), S).0.unwrap();
    assert_eq!((obj(&w).interact_ms, obj(&w).mode), (6_252, 0));
    // Mode 0: T := now + 3,000, mode 1.
    call(&mut w, &inputs(10, 6_253), S).0.unwrap();
    assert_eq!((obj(&w).interact_ms, obj(&w).mode), (9_253, 1));
}

// Covers: specs/world/objects-client.md §26.4, §edge-cases-original-bugs r3, §edge-cases-original-bugs r4
#[test]
fn clientfn_4_vector() {
    let mut w = world((0, 0));
    let (r, out) = call(&mut w, &inputs(4, 100), S);
    assert!(!r.unwrap());
    assert_eq!(obj(&w).interact_ms, 10_851);
    assert_eq!(obj(&w).mode, 3);
    assert_eq!(mode_sounds(&out), 1);
    // Through site A: still one sound call (the function returned 0).
    let mut w = world((0, 0));
    let mut out = Vec::new();
    object_update(&mut w, &inputs(4, 100), S, &mut out).unwrap();
    assert_eq!(mode_sounds(&out), 1);
    // Mode 3 at its last frame (End(3) = 21 × 256 − 256) → mode 0.
    obj_mut(&mut w).frame = 20 * 256;
    call(&mut w, &inputs(4, 100), S).0.unwrap();
    assert_eq!(obj(&w).mode, 0);
    // Already in mode 3 when the timer fires: the frame stays at the raw
    // `Start5` byte.
    let mut i = inputs(4, 20_000);
    i.objclient.rows[0].start[5] = 7;
    obj_mut(&mut w).mode = 3;
    obj_mut(&mut w).frame = 100;
    call(&mut w, &i, S).0.unwrap();
    assert_eq!((obj(&w).mode, obj(&w).frame), (3, 7));
}

// Covers: specs/world/objects-client.md §26.5
#[test]
fn clientfn_5_vector() {
    let mut w = world((0, 0));
    let (r, _) = call(&mut w, &inputs(5, 100), S);
    assert!(!r.unwrap());
    assert_eq!(obj(&w).interact_ms, 31_851);
    assert_eq!(LO2 & 1, 1);
    assert_eq!(obj(&w).mode, 3);
    // Mode 4 at its end → mode 0.
    obj_mut(&mut w).mode = 4;
    obj_mut(&mut w).frame = 20 * 256;
    call(&mut w, &inputs(5, 100), S).0.unwrap();
    assert_eq!(obj(&w).mode, 0);
}

// Covers: specs/world/objects-client.md §26.6
#[test]
fn clientfn_6_vectors() {
    let mut w = world((0, 0));
    obj_mut(&mut w).interact_ms = 1_000;
    obj_mut(&mut w).mode = 3;
    obj_mut(&mut w).frame = 5_119;
    call(&mut w, &inputs(6, 0), S).0.unwrap();
    assert_eq!(obj(&w).mode, 3);
    obj_mut(&mut w).frame = 5_120;
    call(&mut w, &inputs(6, 0), S).0.unwrap();
    assert_eq!(obj(&w).mode, 0);
    // The timer: T := now + range(20000, 30000), mode 3.
    call(&mut w, &inputs(6, 2_000), S).0.unwrap();
    assert_eq!(obj(&w).interact_ms, 2_000 + 20_000 + LO1 % 10_000);
    assert_eq!(obj(&w).mode, 3);
}

// Covers: specs/world/objects-client.md §26.9
#[test]
fn clientfn_9_vectors() {
    // P 27 cells away: distance 27 − (0 / 2 + 2 / 2) = 26.
    let mut w = world((127, 100));
    let mut i = inputs(9, 7_000);
    call(&mut w, &i, S).0.unwrap();
    assert_eq!(obj(&w).interact_ms, 7_000);
    i.now = 8_000;
    assert!(call(&mut w, &i, S).0.unwrap());
    assert_eq!(obj(&w).interact_ms, 7_000);
    // A C smoke: removed at 8,001 (distance 26); returns 0.
    let c = create_client_unit(&mut w, OBJECT, 0, 100, 100).unwrap();
    let cu = ObjUnit {
        key: c,
        client_only: true,
    };
    w.objclient.set_c.get_mut(&c).unwrap().interact_ms = 7_000;
    i.now = 8_001;
    assert!(!call(&mut w, &i, cu).0.unwrap());
    assert!(!w.objclient.set_c.contains_key(&c));
    // Distance 25: kept, T := 8,001.
    let mut w = world((126, 100));
    obj_mut(&mut w).interact_ms = 7_000;
    assert!(call(&mut w, &i, S).0.unwrap());
    assert_eq!(obj(&w).interact_ms, 8_001);
    // No local player: 1.14d reads a null unit.
    let mut w = world((0, 0));
    w.local_player = None;
    obj_mut(&mut w).interact_ms = 1;
    assert!(matches!(
        call(&mut w, &i, S).0,
        Err(HandlerError::Crash { .. })
    ));
}

/// Quest 39 bit 0 set (16 × 39 = byte 78 bit 0), bit 4 as given.
fn rite(set_4: bool) -> [u8; QUEST_RECORD] {
    let mut q = [0u8; QUEST_RECORD];
    q[78] = 1 | if set_4 { 0x10 } else { 0 };
    assert!(quest_bit(&q, 39, 0) && quest_bit(&q, 39, 4) == set_4);
    q
}

// Covers: specs/world/objects-client.md §26.13; specs/client/model.md §8 r7
#[test]
fn clientfn_13_vectors() {
    // Distance 25 − 1 = 24, gate 1 (no current skill).
    let mut w = world((125, 100));
    let mut i = inputs(13, 0);
    i.objclient.quest_flags = Some(rite(false));
    let (r, out) = call(&mut w, &i, S);
    assert!(r.unwrap());
    assert_eq!(w.outgoing, [interact_bytes(2, OBJ.guid)]);
    assert_eq!(
        w.outgoing[0],
        [0x13, 2, 0, 0, 0, 5, 0, 0, 0],
        "9 bytes: 0x13, type u32, GUID u32"
    );
    assert_eq!(obj(&w).mode, 2);
    assert!(w.units[&P].path_stopped);
    assert_eq!(
        w.units[&P].last_mode_request.unwrap().record[..2],
        [2, OBJ.guid as i32]
    );
    assert_eq!(
        out,
        [Output::ObjectFx(ObjFx::GfxRefresh { unit: S, mode: 2 })]
    );
    // Mode 2 now: nothing more.
    call(&mut w, &i, S).0.unwrap();
    assert_eq!(w.outgoing.len(), 1);
    // No local player: nothing (r2 tests it).
    let mut w = world((125, 100));
    w.local_player = None;
    call(&mut w, &i, S).0.unwrap();
    assert!(w.outgoing.is_empty());
    assert_eq!(obj(&w).mode, 0);
    // 39.4 set: nothing.
    let mut w = world((125, 100));
    i.objclient.quest_flags = Some(rite(true));
    call(&mut w, &i, S).0.unwrap();
    assert!(w.outgoing.is_empty());
    assert_eq!(obj(&w).mode, 0);
    // Distance 25: nothing.
    let mut w = world((126, 100));
    i.objclient.quest_flags = Some(rite(false));
    call(&mut w, &i, S).0.unwrap();
    assert!(w.outgoing.is_empty());
    // Gate 0 (a current skill in range without the gate bit, P mode 0):
    // no 0x13, but U still goes to mode 2.
    let mut w = world((125, 100));
    w.units.get_mut(&P).unwrap().skills = Some(SkillList {
        entries: vec![SkillEntry {
            skill: 0,
            mode: 0,
            base: 1,
            level_bonus: 0,
            quantity: 0,
            owner: u32::MAX,
            charges: 0,
            has_charges: false,
        }],
        current: Some(0),
        ..SkillList::default()
    });
    i.tables.skills = vec![Default::default()];
    call(&mut w, &i, S).0.unwrap();
    assert!(w.outgoing.is_empty());
    assert_eq!(obj(&w).mode, 2);
    // No quest record: fatal 0x1F.
    let mut w = world((125, 100));
    i.objclient.quest_flags = None;
    assert!(matches!(
        call(&mut w, &i, S).0,
        Err(HandlerError::Fatal(0x1F))
    ));
}

// Covers: specs/world/objects-client.md §26.17, §27 r1
#[test]
fn clientfn_17_vector() {
    let mut w = world((0, 0));
    obj_mut(&mut w).position = Some((50, 60));
    let i = inputs(17, 0);
    // Latch off: nothing.
    call(&mut w, &i, S).0.unwrap();
    assert!(w.objclient.set_c.is_empty());
    w.objclient.latches.zoo = true;
    call(&mut w, &i, S).0.unwrap();
    assert_eq!(LO1 % 3, 0);
    let chickens: Vec<((u16, u16), u32)> = w
        .objclient
        .set_c
        .values()
        .map(|u| (u.cell(), u.class))
        .collect();
    assert_eq!(chickens, [((50, 60), CHICKEN), ((51, 61), CHICKEN)]);
    assert_eq!(w.units[&P].seed, Some((LO1, 0)));
    let l = w.objclient.latches;
    assert!(l.zoo_spawned);
    assert!(w
        .objclient
        .set_c
        .contains_key(&UnitKey::new(MONSTER, l.last_chicken)));
    // The last chicken lives: nothing.
    let before = w.clone();
    call(&mut w, &i, S).0.unwrap();
    assert_eq!(w, before);
    // Gone: a new wave.
    remove_client_unit(&mut w, UnitKey::new(MONSTER, l.last_chicken));
    call(&mut w, &i, S).0.unwrap();
    assert!(w.objclient.set_c.len() >= 3);
    // The reset clears the four bytes, not the GUID.
    let last = w.objclient.latches.last_chicken;
    w.objclient.latches.reset();
    assert_eq!(w.objclient.latches.last_chicken, last);
    assert!(!w.objclient.latches.zoo);
}

// Covers: specs/world/objects-client.md §26.18
#[test]
fn clientfn_18_vector() {
    let mut w = world((0, 0));
    let (r, out) = call(&mut w, &inputs(18, 1_000), S);
    assert!(r.unwrap());
    assert_eq!(LO1 % 100, 51);
    assert!(out.is_empty());
    assert_eq!(LO2 % 60, 51);
    assert_eq!(obj(&w).interact_ms, 1_000 + 51_000);
    // A seed whose first draw is < 10 mod 100: the request.
    let mut w = world((0, 0));
    let mut s = d2_sim::rng::Seed::new(1, 0);
    let lo = (0..)
        .map(|_| {
            let st = s;
            (st, s.step())
        })
        .find(|&(_, r)| r % 100 < 10)
        .unwrap()
        .0;
    obj_mut(&mut w).seed = Some((lo.lo, lo.hi));
    let (_, out) = call(&mut w, &inputs(18, 1), S);
    assert_eq!(
        out,
        [Output::ObjectSound(ObjSound::Request {
            id: KEEPER_SOUND,
            unit: S
        })]
    );
}

// Covers: specs/world/objects-client.md §26.2, §26.3, §edge-cases-original-bugs r2
#[test]
fn clientfn_2_and_3() {
    let mut w = world((0, 0));
    call(&mut w, &inputs(2, 0), S).0.unwrap();
    assert_eq!(obj(&w).flag_ex, 1);
    assert_eq!(obj(&w).interact_ms, 32);
    assert_eq!(obj(&w).mode, 0);
    // Mode 2: one draw, T += lo & 0x3F, mode 0.
    obj_mut(&mut w).mode = 2;
    call(&mut w, &inputs(2, 0), S).0.unwrap();
    assert_eq!(obj(&w).interact_ms, 33 + (LO2 & 0x3F));
    assert_eq!(obj(&w).mode, 0);
    // Expansion: flag-ex already has 0x2000000, so rule 1 never runs;
    // T 0 → mode 1.
    let mut w = world((0, 0));
    obj_mut(&mut w).flag_ex = FLAG_EX_EXPANSION;
    let (_, out) = call(&mut w, &inputs(3, 0), S);
    assert_eq!(obj(&w).flag_ex, FLAG_EX_EXPANSION);
    assert_eq!((obj(&w).mode, obj(&w).interact_ms), (1, 1));
    assert_eq!(obj(&w).seed, Some(INIT_SEED));
    // ClientFn 3 makes its own sound call.
    assert_eq!(mode_sounds(&out), 1);
}

// Covers: specs/world/objects-client.md §26.8, §26.12, §27 r1, §27 r2
#[test]
fn preload_latches() {
    let mut w = world((0, 0));
    let (_, out) = call(&mut w, &inputs(8, 0), S);
    assert!(out.is_empty(), "mode 0: no preload");
    obj_mut(&mut w).mode = 1;
    let (_, out) = call(&mut w, &inputs(8, 0), S);
    assert_eq!(
        out,
        [Output::ObjectFx(ObjFx::GfxLoad {
            class: 211,
            flag: 1
        })]
    );
    let (_, out) = call(&mut w, &inputs(8, 0), S);
    assert!(out.is_empty(), "once per latch reset");
    let (_, out) = call(&mut w, &inputs(12, 0), S);
    assert_eq!(
        out,
        [537, 538, 539].map(|class| Output::ObjectFx(ObjFx::GfxLoad { class, flag: 0 }))
    );
    w.objclient.latches.reset();
    let (_, out) = call(&mut w, &inputs(8, 0), S);
    assert_eq!(out.len(), 1);
}

fn env(kind: i32) -> Environment {
    Environment {
        index: 0,
        kind,
        ticks: 0,
        intensity: 0,
        created_ms: 0,
        r: 0,
        g: 0,
        b: 0,
        s: 0.0,
        speed: 0,
        eclipse: false,
    }
}

// Covers: specs/world/objects-client.md §26.14, §edge-cases-original-bugs r8
#[test]
fn clientfn_14_bonfire() {
    let mut w = world((0, 0));
    w.act = Some(ActLoad {
        act: 0,
        init_seed: 0,
        town_level: 1,
        f8: 0,
    });
    w.environment = Some(env(2));
    let mut i = inputs(14, 10);
    i.objclient.rows[0].env_effect = 1;
    i.objclient.rows[0].lit = [0, 19, 19, 0, 0, 0, 0, 0];
    i.objclient.rows[0].rgb = (255, 236, 176);
    let (_, out) = call(&mut w, &i, S);
    assert_eq!(obj(&w).mode, 1);
    assert_eq!(obj(&w).interact_ms, 510);
    assert_eq!(obj(&w).flag_2, Some(false));
    assert_eq!(
        out,
        [
            Output::ObjectFx(ObjFx::GfxRefresh { unit: S, mode: 1 }),
            Output::ObjectFx(ObjFx::Light {
                unit: S,
                lit: 19,
                rgb: (255, 236, 176)
            })
        ]
    );
    // Within 500 ms: nothing.
    i.now = 510;
    assert!(call(&mut w, &i, S).1.is_empty());
    // Day (p 0): mode 1 → 0, light `Lit0` 0.
    w.environment = Some(env(0));
    i.now = 511;
    let (_, out) = call(&mut w, &i, S);
    assert_eq!(obj(&w).mode, 0);
    assert_eq!(out.len(), 2);
    // Day again, mode 0: nothing but the re-arm.
    i.now = 2_000;
    assert!(call(&mut w, &i, S).1.is_empty());
    assert_eq!(obj(&w).interact_ms, 2_500);
    // p > 3: fatal 0x66; no act: fatal 0x547.
    w.environment = Some(env(4));
    i.now = 3_000;
    assert!(matches!(
        call(&mut w, &i, S).0,
        Err(HandlerError::Fatal(0x66))
    ));
    w.act = None;
    assert!(matches!(
        call(&mut w, &i, S).0,
        Err(HandlerError::Fatal(0x547))
    ));
    // `EnvEffect` 0: no change.
    i.objclient.rows[0].env_effect = 0;
    assert!(call(&mut w, &i, S).1.is_empty());
}

// Covers: specs/world/objects-client.md §26.15, §edge-cases-original-bugs r6
#[test]
fn clientfn_15_overlay() {
    let mut w = world((0, 0));
    let create = Output::ObjectFx(ObjFx::OverlayCreate {
        unit: S,
        overlay: OVERLAY_NPCALERT,
        kind: OVERLAY_KIND_LOOP,
    });
    assert_eq!(
        call(&mut w, &inputs(15, 1), S).1,
        std::slice::from_ref(&create)
    );
    assert!(call(&mut w, &inputs(15, 501), S).1.is_empty());
    // Every 500 ms in mode 0 another record (no duplicate test).
    assert_eq!(call(&mut w, &inputs(15, 502), S).1, [create]);
    obj_mut(&mut w).mode = 1;
    assert_eq!(
        call(&mut w, &inputs(15, 1_003), S).1,
        [Output::ObjectFx(ObjFx::OverlayRemove {
            unit: S,
            overlay: OVERLAY_NPCALERT
        })]
    );
}

// Covers: specs/world/objects-client.md §26.16, §edge-cases-original-bugs r7
#[test]
fn clientfn_16_portal() {
    let mut w = world((0, 0));
    let mut i = inputs(16, 0);
    i.objclient.rows[0].start[1] = 2;
    obj_mut(&mut w).mode = 1;
    obj_mut(&mut w).frame = 2 * 256;
    call(&mut w, &i, S).0.unwrap();
    assert_eq!((obj(&w).mode, obj(&w).frame), (2, 0));
    // Through site A with speed 256 the generic step moves the frame
    // first: the rule does not hold; the generic step reaches mode 2 at
    // the end of mode 1 (PROVISIONAL, REC-45).
    let mut w = world((0, 0));
    i.objclient.rows[0].start[1] = 0;
    i.objclient.rows[0].frame_delta = [256; 8];
    i.objclient.rows[0].frame_cnt = [2 * 256; 8];
    obj_mut(&mut w).mode = 1;
    object_update(&mut w, &i, S, &mut Vec::new()).unwrap();
    assert_eq!((obj(&w).mode, obj(&w).frame), (1, 256));
    object_update(&mut w, &i, S, &mut Vec::new()).unwrap();
    assert_eq!((obj(&w).mode, obj(&w).frame), (2, 0));
}

// Covers: specs/world/objects-client.md §26.6 text, §25 r5
#[test]
fn generic_step_clamps_a_non_cycling_mode() {
    let mut w = world((0, 0));
    let mut i = inputs(0, 0);
    i.objclient.rows[0].frame_delta = [200; 8];
    i.objclient.rows[0].frame_cnt = [3 * 256; 8];
    i.objclient.rows[0].cycle_anim[0] = 1;
    obj_mut(&mut w).mode = 3;
    // 200, 400, 600, then 800 ≥ 768 → End(3) = 512; the next step adds
    // the speed again (712 < 768: no clamp).
    let mut frames = Vec::new();
    for _ in 0..5 {
        object_update(&mut w, &i, S, &mut Vec::new()).unwrap();
        frames.push(obj(&w).frame);
    }
    assert_eq!(frames, [200, 400, 600, 512, 712]);
    assert_eq!(obj(&w).mode, 3);
    // A cycling mode wraps (PROVISIONAL).
    obj_mut(&mut w).mode = 0;
    obj_mut(&mut w).frame = 700;
    object_update(&mut w, &i, S, &mut Vec::new()).unwrap();
    assert_eq!(obj(&w).frame, 900 - 768);
}

// Covers: specs/client/model.md §8 r7
#[test]
fn interact_sender_cases() {
    let mut w = world((0, 0));
    let mon = UnitKey::new(MONSTER, 9);
    w.units.insert(mon, ClientUnit::new(mon));
    let mut i = ModelInputs {
        now: 150,
        ..ModelInputs::default()
    };
    // Monster: now − 0 < 200 → nothing.
    interact::send(&mut w, &i, 1, 9).unwrap();
    assert!(w.outgoing.is_empty());
    i.now = 200;
    interact::send(&mut w, &i, 1, 9).unwrap();
    assert_eq!(w.outgoing, [interact_bytes(1, 9)]);
    assert_eq!(w.units[&mon].interact_ms, 200);
    interact::send(&mut w, &i, 1, 9).unwrap();
    assert_eq!(w.outgoing.len(), 1);
    // Player: P faces U, 0x13.
    let other = UnitKey::new(PLAYER, 2);
    w.units.insert(other, ClientUnit::new(other));
    interact::send(&mut w, &i, 0, 2).unwrap();
    assert_eq!(w.units[&P].turned_toward, Some(other));
    assert_eq!(w.outgoing[1], interact_bytes(0, 2));
    // Not in S: nothing.
    interact::send(&mut w, &i, 2, 77).unwrap();
    assert_eq!(w.outgoing.len(), 2);
    // Object with flag 0x4, class 404 in mode 0 without `qf2 `: the
    // player event sound, nothing sent.
    let mut list = SkillList::default();
    list.entries.push(SkillEntry {
        skill: 1,
        mode: 0,
        base: 1,
        level_bonus: 0,
        quantity: 0,
        owner: u32::MAX,
        charges: 0,
        has_charges: false,
    });
    w.units.get_mut(&P).unwrap().skills = Some(list);
    let o = obj_mut(&mut w);
    o.flag_4 = true;
    o.class = 404;
    let out = interact::send(&mut w, &i, 2, OBJ.guid).unwrap();
    assert_eq!(
        out,
        [Output::ObjectSound(ObjSound::PlayerEvent {
            player: P,
            event: 0x13
        })]
    );
    assert_eq!(w.outgoing.len(), 2);
    // Another class: the skill start then 0x13.
    obj_mut(&mut w).class = 3;
    let out = interact::send(&mut w, &i, 2, OBJ.guid).unwrap();
    assert_eq!(
        out,
        [Output::ObjectFx(ObjFx::SkillStart {
            player: P,
            record: [1, -1, 2, OBJ.guid as i32, 0, 0, 0]
        })]
    );
    assert_eq!(w.outgoing[2], interact_bytes(2, OBJ.guid));
    // Item: C→S 0x16 {4, GUID, b}.
    let item = UnitKey::new(4, 30);
    w.units.insert(item, ClientUnit::new(item));
    i.objclient.pickup_flag = 1;
    interact::send(&mut w, &i, 4, 30).unwrap();
    assert_eq!(w.outgoing[3], [0x16, 4, 0, 0, 0, 30, 0, 0, 0, 1, 0, 0, 0]);
}

// The player code 0x02 row of `client/model.md` §8 rule 4 (one row of
// the table, so no claim on the whole rule).
#[test]
fn player_mode_request_code_2_sends_the_interact() {
    use super::super::receive::receive_chunk;
    let mut w = world((0, 0));
    w.in_game = true;
    w.expansion = 1;
    // S→C 0x0E on P: code u8@6 = 2, record {type u8@7 = 2, GUID u32@8}.
    let msg = [0x0E, 0, 1, 0, 0, 0, 2, 2, 5, 0, 0, 0];
    let mut log = ReceiveLog::default();
    let mut out = Vec::new();
    let d = Dispatch::from_spec().unwrap();
    let i = ModelInputs::default();
    receive_chunk(&mut w, &i, &d, &mut log, &mut out, &msg).unwrap();
    assert!(w.outgoing.is_empty(), "queued until the update pass");
    update_pass(&mut w, &i, &d, &mut log, &mut out);
    assert!(log.rejected.is_empty(), "{:?}", log.rejected);
    assert_eq!(w.units[&P].last_mode_request.unwrap().code, 2);
    assert_eq!(w.outgoing, [interact_bytes(2, OBJ.guid)]);
    // The creation field of an expansion game (§2 rule 6) through a C
    // create.
    let c = create_client_unit(&mut w, OBJECT, 0, 0, 0).unwrap();
    assert_eq!(w.objclient.set_c[&c].flag_ex, FLAG_EX_EXPANSION);
}

/// S→C 0x50 code 36 turns the zoo latch on (`client/msg-ui.md` §7 r2,
/// code 36 row) and still emits its `QuestSpecial`.
#[test]
fn quest_special_36_sets_the_zoo_latch() {
    use super::super::receive::receive_chunk;
    let mut w = ClientWorld::default();
    let msg = [0x50, 36, 0, 7, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    let mut log = ReceiveLog::default();
    let mut out = Vec::new();
    let d = Dispatch::from_spec().unwrap();
    receive_chunk(
        &mut w,
        &ModelInputs::default(),
        &d,
        &mut log,
        &mut out,
        &msg,
    )
    .unwrap();
    assert!(log.rejected.is_empty(), "{:?}", log.rejected);
    assert!(w.objclient.latches.zoo);
    assert_eq!(w.objclient.latches.zoo_word, 7);
    assert!(matches!(out[..], [Output::QuestSpecial { code: 36, .. }]));
}
