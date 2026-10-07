// Spec: specs/client/msg-ui.md
//! The UI dispatch of the 0x5D, 0x63 and 0x77 outputs (`msg-ui.md` Test
//! vectors, UI side; "A" = `20261006-015956`, "B" = `-022633`).

use super::*;
use crate::bridge::world::{ClientUnit, UnitKey, PLAYER};
use crate::ui::layout::Screen;
use crate::ui::original::{UiConfig, UiOutcome};
use crate::ui::states::UiEffect;

fn world(expansion: bool) -> ClientWorld {
    let mut w = ClientWorld::default();
    let key = UnitKey::new(PLAYER, 1);
    let mut u = ClientUnit::new(key);
    u.mode = 1;
    w.units.insert(key, u);
    w.local_player = Some(key);
    w.expansion = u32::from(expansion);
    w
}

fn ui() -> OriginalUi {
    let config = UiConfig {
        screen: Screen::R800,
        expansion_installed: true,
    };
    OriginalUi::new(config, None).unwrap()
}

fn quest(c: u8, f: u8, s: u8, v: i16) -> Output {
    Output::QuestUi {
        chain: c,
        flags: f,
        status: s,
        extra: v,
    }
}

fn ui_sounds(ids: &[i32]) -> Vec<SoundRequest> {
    ids.iter().map(|&i| SoundRequest::Ui(i)).collect()
}

// Covers: specs/client/msg-ui.md §1 r2, §1 r5
#[test]
fn quest_sounds() {
    let w = world(false);
    let mut u = ui();
    for (c, f, v) in [
        (8, 2, 0),
        (0x21, 0x10, 5),
        (33, 1, 0),
        (4, 2, 0),
        (10, 0x10, 0),
    ] {
        u.apply_output(&quest(c, f, 0, v), &w).unwrap();
    }
    assert_eq!(
        u.take_outcome().sounds,
        ui_sounds(&[7, 5, 237, 241, 2456, 2474])
    );
    // f = 3, c = 8: the bit-0 table has no 8 (edge case 1).
    u.apply_output(&quest(8, 3, 0, 0), &w).unwrap();
    assert_eq!(u.take_outcome(), UiOutcome::default());
    // f bit 0 with c = 4 is a model row: nothing at delivery.
    u.apply_output(&quest(4, 1, 0, 0), &w).unwrap();
    assert_eq!(u.take_outcome(), UiOutcome::default());
}

// Covers: specs/client/msg-ui.md §1 r6
#[test]
fn quest_log_tail_a129118() {
    let w = world(false);
    let mut u = ui();
    u.apply_output(&quest(1, 0, 1, 0), &w).unwrap();
    assert!(u.is_open(UI_QUESTLOG as u8));
    assert!(u.msg_state().quest_log_latch);
    let o = u.take_outcome();
    assert!(o.effects.contains(&UiEffect::Opened(UI_QUESTLOG as u8)));
    // The entry of c in the quest-log table: not in the specs.
    assert_eq!(o.skipped, [skip::QUEST_LOG_TABLE]);
    // Latch set, quest screen closed: nothing.
    u.apply_output(&quest(1, 0, 2, 0), &w).unwrap();
    assert_eq!(u.take_outcome(), UiOutcome::default());
    // f bit 5, c 32: `[0x007BF2AC]` := v, then the tail.
    u.apply_output(&quest(32, 0x20, 0, -3), &w).unwrap();
    assert_eq!(u.msg_state().quest_7bf2ac, Some(-3));
}

// Covers: specs/client/msg-ui.md §1 r2, §1 r7
#[test]
fn quest_parts_without_a_spec_are_skipped() {
    let w = world(false);
    let mut u = ui();
    for (c, f) in [(3, 1), (13, 1), (15, 1), (36, 2), (5, 0x20)] {
        u.apply_output(&quest(c, f, 0, 0), &w).unwrap();
    }
    assert_eq!(
        u.take_outcome().skipped,
        [
            skip::SCREEN_MESSAGE,
            skip::MONSTER_EFFECT,
            skip::SCREEN_MESSAGE,
            skip::VIDEO_7,
            skip::DEN_COUNTER
        ]
    );
    // f bit 1, c 23: expansion → `[0x007BC9D8]`; classic → the video
    // path (skipped) and `[0x007BC9D4]`.
    u.apply_output(&quest(23, 2, 0, 0), &world(true)).unwrap();
    assert!(u.msg_state().act_end_7bc9d8 && !u.msg_state().act_end_7bc9d4);
    let mut u = ui();
    u.apply_output(&quest(23, 2, 0, 0), &w).unwrap();
    assert!(!u.msg_state().act_end_7bc9d8 && u.msg_state().act_end_7bc9d4);
    assert_eq!(u.take_outcome().skipped, [skip::ACT_END_VIDEO]);
}

const B7852: [u8; 16] = [2, 1, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];

// Covers: specs/client/msg-ui.md §2 r2
#[test]
fn waypoint_menu_b7852() {
    let w = world(false);
    let mut u = ui();
    let o = Output::WaypointMenu {
        guid: 0x0A,
        record: B7852,
    };
    u.apply_output(&o, &w).unwrap();
    assert!(u.is_open(UI_WAYPOINT as u8));
    let st = u.msg_state().waypoint.unwrap();
    assert_eq!((st.guid, st.close_latch), (0x0A, false));
    assert_eq!(st.record.0, B7852, "magic 0x0102 kept, bit 0 already set");
    // No room in the model: tab 0.
    assert_eq!(st.tab, Some(0));
    let out = u.take_outcome();
    assert_eq!(out.skipped, [skip::INPUT_RESET, skip::WAYPOINT_ROWS]);
    // Jump 1 (§4.3) with the left slot: the cursor effect is reported.
    assert!(out
        .effects
        .iter()
        .any(|e| matches!(e, UiEffect::CursorX(_))));
    // Magic 0x0000: wiped to index 0 only.
    let mut u = ui();
    let mut r = B7852;
    r[..2].copy_from_slice(&[0, 0]);
    u.apply_output(&Output::WaypointMenu { guid: 1, record: r }, &w)
        .unwrap();
    let mut want = [0u8; 16];
    want[2] = 1;
    assert_eq!(u.msg_state().waypoint.unwrap().record.0, want);
    // Another magic: fatal in 1.14d.
    let mut u = ui();
    r[..2].copy_from_slice(&[7, 7]);
    assert!(matches!(
        u.apply_output(&Output::WaypointMenu { guid: 1, record: r }, &w),
        Err(OriginalUiError::Waypoint(_))
    ));
}

// Covers: specs/client/msg-ui.md §2 r2
#[test]
fn waypoint_menu_refused_stores_nothing() {
    let w = world(false);
    let mut u = ui();
    let menu = |guid| Output::WaypointMenu {
        guid,
        record: B7852,
    };
    u.apply_output(&menu(0x0A), &w).unwrap();
    // Open already: the gate refuses (`ui-states.tsv` row 20), so the
    // second menu's GUID and record are not stored.
    let mut r = B7852;
    r[3] = 0x80;
    u.apply_output(
        &Output::WaypointMenu {
            guid: 0x0B,
            record: r,
        },
        &w,
    )
    .unwrap();
    let st = u.msg_state().waypoint.unwrap();
    assert_eq!((st.guid, st.record.0), (0x0A, B7852));
    // A dead player: the call returns 1 without setting the flag
    // (`ui/panels.md` §2.5), so the record is stored.
    let mut dead = world(false);
    dead.units.get_mut(&UnitKey::new(PLAYER, 1)).unwrap().mode = 0;
    let mut u = ui();
    u.apply_output(&menu(0x0C), &dead).unwrap();
    assert!(!u.is_open(UI_WAYPOINT as u8));
    assert_eq!(u.msg_state().waypoint.map(|s| s.guid), Some(0x0C));
}

// Covers: specs/client/msg-ui.md §3 r2
#[test]
fn stash_and_cube_a104179() {
    let w = world(false);
    let mut u = ui();
    u.apply_output(&Output::TradeAction { code: 0x10 }, &w)
        .unwrap();
    assert!(u.is_open(UI_STASH as u8));
    assert_eq!(u.msg_state().inventory_mode, MODE_STASH);
    // 0x11: the stash closes (inventory mode 0x0C).
    u.apply_output(&Output::TradeAction { code: 0x11 }, &w)
        .unwrap();
    assert!(!u.is_open(UI_STASH as u8));
    assert_eq!(u.msg_state().inventory_mode, 0);
    // 0x15: the cube.
    u.apply_output(&Output::TradeAction { code: 0x15 }, &w)
        .unwrap();
    assert!(u.is_open(UI_CUBE as u8));
    assert_eq!(u.msg_state().inventory_mode, MODE_CUBE);
    // 0x11 with the cube's mode: nothing.
    u.apply_output(&Output::TradeAction { code: 0x11 }, &w)
        .unwrap();
    assert!(u.is_open(UI_CUBE as u8));
}

// Covers: specs/client/msg-ui.md §3 r2, §3 r3
#[test]
fn trade_codes() {
    let w = world(false);
    let mut u = ui();
    // 0x0C with trade state 0: close trade(0), no decline.
    u.apply_output(&Output::TradeAction { code: 0x0C }, &w)
        .unwrap();
    assert_eq!(u.take_outcome(), UiOutcome::default());
    u.apply_output(&Output::TradeAction { code: 0x0E }, &w)
        .unwrap();
    assert!(u.msg_state().trade_7bce28);
    u.apply_output(&Output::TradeAction { code: 0x0F }, &w)
        .unwrap();
    assert!(!u.msg_state().trade_7bce28);
    // Code 9: player event sound 23 on the local player.
    u.apply_output(&Output::TradeAction { code: 0x09 }, &w)
        .unwrap();
    assert_eq!(
        u.take_outcome().sounds,
        [SoundRequest::PlayerEvent {
            unit: UnitKey::new(PLAYER, 1),
            event: 23
        }]
    );
    // Past 0x15 and the "nothing" codes.
    for code in [0x16, 0xFF, 0x03, 0x12] {
        u.apply_output(&Output::TradeAction { code }, &w).unwrap();
    }
    assert_eq!(u.take_outcome(), UiOutcome::default());
    // The trade codes need the trade helpers: skipped.
    u.apply_output(&Output::TradeAction { code: 0x00 }, &w)
        .unwrap();
    assert_eq!(u.take_outcome().skipped, [skip::TRADE]);
    // A sound output is the audio layer's.
    let s = Output::ServerSound {
        unit: UnitKey::new(PLAYER, 1),
        class: 0,
        event: 2,
    };
    u.apply_output(&s, &w).unwrap();
    assert_eq!(u.take_outcome(), UiOutcome::default());
}
