//! Tests of the client mode machines (`client/model.md` §8).

use super::dispatch::HandlerError;
use super::drlg::DrlgRoomId;
use super::modes::{
    mode_request, monster_mode, player_mode as pm, FATAL_OBJECT_CODE, FATAL_PLAYER_CODE,
};
use super::objects::{ObjSound, ObjUnit, NO_LOCAL_DISTANCE};
use super::output::{Output, Outputs};
use super::world::{
    ActiveRoom, ClientUnit, ClientWorld, ModelInputs, ObjectRow, UnitKey, ITEM, MISSILE, MONSTER,
    OBJECT, PLAYER,
};

const P: UnitKey = UnitKey::new(PLAYER, 1);

fn world_with(key: UnitKey, mode: u32) -> ClientWorld {
    let mut w = ClientWorld::default();
    let mut u = ClientUnit::new(key);
    u.mode = mode;
    u.position = Some((50, 50));
    w.units.insert(key, u);
    w
}

fn req(
    w: &mut ClientWorld,
    i: &ModelInputs,
    key: UnitKey,
    code: u8,
    r: [i32; 7],
) -> Result<Vec<Output>, HandlerError> {
    let out = Outputs::default();
    mode_request(w, i, key, code, r, &out)?;
    Ok(out.take())
}

const R0: [i32; 7] = [0, 0, 0x2A, 0, 0, 0, 0];

// Covers: specs/client/model.md §8 r4, §8 r3
#[test]
fn player_codes_set_the_table_modes() {
    let i = ModelInputs::default();
    for (code, mode) in [
        (0x00, pm::WALK),
        (0x01, pm::WALK),
        (0x06, pm::GET_HIT),
        (0x07, pm::NEUTRAL),
        (0x08, pm::DEATH),
        (0x09, pm::DEAD),
        (0x14, pm::SEQUENCE),
        (0x17, pm::RUN),
        (0x18, pm::RUN),
        (0x19, pm::BLOCK),
    ] {
        let mut w = world_with(P, 1);
        w.units.get_mut(&P).unwrap().mode = 7;
        req(&mut w, &i, P, code, R0).unwrap();
        let u = &w.units[&P];
        assert_eq!(u.mode, mode, "code {code:#x}");
        assert_eq!(u.last_mode_request.map(|m| m.code), Some(code));
    }
    // Codes that keep the mode: 0x02, 0x12 (no inventory input), 0x13,
    // 0x15, 0x16.
    for code in [0x02, 0x12, 0x13, 0x15, 0x16] {
        let mut w = world_with(P, 7);
        req(&mut w, &i, P, code, R0).unwrap();
        assert_eq!(w.units[&P].mode, 7, "code {code:#x}");
    }
}

// Covers: specs/client/model.md §8 r4
#[test]
fn player_hit_class_flag_and_path_stop() {
    let i = ModelInputs::default();
    for code in [0x06, 0x08, 0x12, 0x13, 0x14] {
        let mut w = world_with(P, 1);
        req(&mut w, &i, P, code, R0).unwrap();
        assert_eq!(w.units[&P].hit_class, 0x2A, "code {code:#x}");
    }
    let mut w = world_with(P, 1);
    req(&mut w, &i, P, 0x07, R0).unwrap();
    assert_eq!(w.units[&P].flag_2, Some(true));
    // A unit in mode 0x13 with flag 1: its path is stopped first.
    let mut w = world_with(P, pm::SEQUENCE);
    req(&mut w, &i, P, 0x17, R0).unwrap();
    assert!(w.units[&P].path_stopped);
    let mut w = world_with(P, 1);
    req(&mut w, &i, P, 0x17, R0).unwrap();
    assert!(!w.units[&P].path_stopped);
}

/// Code 7 on a dead player places it at (r0, r1) (PROVISIONAL REC-279:
/// the respawn's 0x0D, after the 0x15 the dead player ignored); a living
/// player stays where it is, and a dead one with record (0, 0) too.
// Covers: specs/client/model.md §8 r4
#[test]
fn code_7_places_a_player_that_was_dead() {
    let i = ModelInputs::default();
    let back = [103, 23, 0, 0, 0, 0, 0];
    for dead in [pm::DEAD, pm::DEATH] {
        let mut w = world_with(P, dead);
        req(&mut w, &i, P, 0x07, back).unwrap();
        assert_eq!(w.units[&P].position, Some((103, 23)), "mode {dead}");
        assert_eq!(w.units[&P].mode, pm::NEUTRAL);
    }
    let mut w = world_with(P, pm::NEUTRAL);
    req(&mut w, &i, P, 0x07, back).unwrap();
    assert_eq!(w.units[&P].position, Some((50, 50)));
    let mut w = world_with(P, pm::DEAD);
    req(&mut w, &i, P, 0x07, R0).unwrap();
    assert_eq!(w.units[&P].position, Some((50, 50)));
}

// Covers: specs/client/model.md §8 r4
#[test]
fn player_invalid_codes_are_fatal_0x432() {
    let i = ModelInputs::default();
    for code in [3u8, 4, 5, 0x0A, 0x0F, 0x11, 0x1A, 0xFF] {
        let mut w = world_with(P, 1);
        assert_eq!(
            req(&mut w, &i, P, code, R0),
            Err(HandlerError::Fatal(FATAL_PLAYER_CODE)),
            "code {code:#x}"
        );
    }
}

// Covers: specs/client/model.md §8 r4
#[test]
fn town_walk_and_neutral() {
    let i = ModelInputs::default();
    let room = DrlgRoomId(3);
    for (level, walk, neutral) in [
        (1u16, pm::TOWN_WALK, pm::TOWN_NEUTRAL),
        (2, pm::WALK, pm::NEUTRAL),
    ] {
        let mut w = world_with(P, 1);
        w.active_rooms = Some(vec![ActiveRoom {
            x0: 0,
            y0: 0,
            w: 100,
            h: 100,
            level,
            room,
        }]);
        w.room_units.place(P, Some(room));
        req(&mut w, &i, P, 0x00, R0).unwrap();
        assert_eq!(w.units[&P].mode, walk);
        req(&mut w, &i, P, 0x07, R0).unwrap();
        assert_eq!(w.units[&P].mode, neutral);
    }
}

fn object_inputs() -> ModelInputs {
    let mut i = ModelInputs::default();
    let row = ObjectRow {
        lit: [0, 0, 8, 0, 0, 0, 0, 0],
        rgb: (255, 128, 0),
        ..ObjectRow::default()
    };
    i.tables.objects = vec![row; 4];
    i
}

// Covers: specs/client/model.md §8 r5, §18 r2
#[test]
fn object_code_3_mode_light_and_sound() {
    let i = object_inputs();
    let o = UnitKey::new(OBJECT, 9);
    let mut w = world_with(o, 0);
    w.units.get_mut(&o).unwrap().class = 2;
    let out = req(&mut w, &i, o, 3, [1, 2, 0, 0, 0, 0, 0]).unwrap();
    assert_eq!(w.units[&o].mode, 2);
    assert_eq!(w.lights.len(), 1);
    assert_eq!(
        out,
        [Output::ObjectSound(ObjSound::Mode {
            unit: ObjUnit {
                key: o,
                client_only: false
            },
            class: 2,
            mode: 2,
            local_dist: NO_LOCAL_DISTANCE,
        })]
    );
    // Back to a mode with `Lit` 0: the light is removed.
    req(&mut w, &i, o, 3, [1, 0, 0, 0, 0, 0, 0]).unwrap();
    assert!(w.lights.is_empty());
    // Code 0x15 changes nothing here (the shrine use is the handler's);
    // any other code is fatal 0x39C.
    assert!(req(&mut w, &i, o, 0x15, R0).unwrap().is_empty());
    assert_eq!(
        req(&mut w, &i, o, 2, R0),
        Err(HandlerError::Fatal(FATAL_OBJECT_CODE))
    );
}

// Covers: specs/client/model.md §8 r6, §8 r1
#[test]
fn item_code_2_and_missiles() {
    let i = ModelInputs::default();
    let it = UnitKey::new(ITEM, 4);
    let mut w = world_with(it, 0);
    req(&mut w, &i, it, 2, [1, 3, 0, 0, 0, 0, 0]).unwrap();
    assert_eq!((w.units[&it].mode, w.units[&it].flag_2), (3, Some(true)));
    req(&mut w, &i, it, 2, [0, 5, 0, 0, 0, 0, 0]).unwrap();
    assert_eq!((w.units[&it].mode, w.units[&it].flag_2), (5, Some(false)));
    req(&mut w, &i, it, 7, [0, 1, 0, 0, 0, 0, 0]).unwrap();
    assert_eq!(w.units[&it].mode, 5);
    // A missile's request returns at once and is not stored.
    let m = UnitKey::new(MISSILE, 2);
    let mut w = world_with(m, 0);
    req(&mut w, &i, m, 1, R0).unwrap();
    assert_eq!(w.units[&m].last_mode_request, None);
}

// Covers: specs/client/model.md §8 r2
#[test]
fn monster_mode_is_the_mode_its_code_was_sent_for() {
    // PROVISIONAL reading of open question 1 (`sim/intents-events.md`
    // §7.4 mode table).
    assert_eq!(monster_mode(8), Some(0));
    assert_eq!(monster_mode(7), Some(1));
    assert_eq!(monster_mode(1), Some(2));
    assert_eq!(monster_mode(0), Some(2));
    assert_eq!(monster_mode(9), Some(12));
    assert_eq!(monster_mode(12), Some(8));
    assert_eq!(monster_mode(20), Some(13));
    assert_eq!(monster_mode(23), Some(15));
    assert_eq!(monster_mode(2), None);
    let i = ModelInputs::default();
    let k = UnitKey::new(MONSTER, 5);
    let mut w = world_with(k, 1);
    req(&mut w, &i, k, 9, R0).unwrap();
    assert!(w.units[&k].is_dead());
    req(&mut w, &i, k, 2, R0).unwrap();
    assert_eq!(w.units[&k].mode, 12);
}

/// PROVISIONAL (client/model.md OQ 1; REC-51): codes 0x15 / 0x16 (S→C
/// 0x4D / 0x4C) set the skill's `anim` (player) or `monanim` (monster)
/// mode; a skill without a row leaves the mode.
// Covers: specs/client/msg-units.md §4 r1; specs/client/model.md §8 r1
#[test]
fn skill_codes_set_the_skill_animation_mode() {
    use super::world::SkillRow;
    let mut i = ModelInputs::default();
    i.tables.skills = vec![
        SkillRow::default(),
        SkillRow {
            anim: 10,
            monanim: 4,
            ..SkillRow::default()
        },
    ];
    let m = UnitKey::new(MONSTER, 2);
    for (key, code, want) in [(P, 0x16, 10), (P, 0x15, 10), (m, 0x16, 4), (m, 0x15, 4)] {
        let mut w = world_with(key, 1);
        req(&mut w, &i, key, code, [1, -1, 0, 0, 0, 0, 0]).unwrap();
        assert_eq!(w.units[&key].mode, want, "{key:?} {code:#x}");
        // No row: unchanged.
        let mut w = world_with(key, 1);
        req(&mut w, &i, key, code, [9, -1, 0, 0, 0, 0, 0]).unwrap();
        assert_eq!(w.units[&key].mode, 1);
    }
}
