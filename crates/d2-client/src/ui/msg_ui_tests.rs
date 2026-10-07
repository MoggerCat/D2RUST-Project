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

fn npc_text(t: u8, count: u8, kind0: u8, str0: u16) -> Output {
    let mut bytes = [0u8; 40];
    bytes[0] = 0x27;
    bytes[1] = t;
    bytes[2] = 6;
    bytes[6] = count;
    bytes[8] = kind0;
    bytes[10..12].copy_from_slice(&str0.to_le_bytes());
    Output::NpcText {
        bytes,
        present: true,
        object_class: 0,
    }
}

fn npc_dialog(cursor_item: bool) -> NpcDialog {
    NpcDialog {
        kind: 1,
        guid: 6,
        quest_flags: [0; 96],
        unit: UnitKey::new(1, 6),
        class: 148,
        interact: true,
        f4b1a10: None,
        cursor_item,
        npc_monsters: vec![UnitKey::new(1, 6)],
    }
}

// Covers: specs/client/msg-ui.md §5 r2
#[test]
fn npc_text_list() {
    let w = world(false);
    let mut u = ui();
    assert_eq!(u.npc_text(), None);
    // A seq 37351: type 1, one entry (kind 0, string 0x25): rebuilt.
    u.apply_output(&npc_text(1, 1, 0, 0x25), &w).unwrap();
    let l = *u.npc_text().unwrap();
    assert_eq!((l.count(), l.kind(0), l.string(0)), (1, 0, 0x25));
    assert_eq!(l.first_m(), Some(0x25));
    // r2.1 overhead number and r2.2 type 2: the list stays.
    u.apply_output(&npc_text(1, 1, 3, 37), &w).unwrap();
    u.apply_output(&npc_text(2, 1, 0, 9), &w).unwrap();
    assert_eq!(u.npc_text(), Some(&l));
    assert_eq!(u.take_outcome().skipped, [skip::NPC_TEXT_SHOW; 3]);
    // r2.3: any other type frees it.
    u.apply_output(&npc_text(4, 1, 0, 9), &w).unwrap();
    assert_eq!(u.npc_text(), None);
}

// Covers: specs/client/msg-ui.md §16 r4
#[test]
fn npc_dialog_case() {
    use crate::bridge::msg::ui_npc::DialogCase;
    let list = |count, kind0, str0| match npc_text(1, count, kind0, str0) {
        Output::NpcText { bytes, .. } => NpcTextList::from_record(&bytes),
        _ => unreachable!(),
    };
    let d = npc_dialog(false);
    let l = list(1, 0, 0x25);
    // B0 first, even without a list.
    assert_eq!(dialog_case(1, None, &d).unwrap(), Some(DialogCase::B0));
    // No list: fatal 0x1060.
    assert!(matches!(
        dialog_case(0, None, &d),
        Err(OriginalUiError::NoNpcText)
    ));
    // B1 before B2; B2 with m; m = 0xFFFF → B3–B6.
    assert_eq!(
        dialog_case(0, Some(&l), &npc_dialog(true)).unwrap(),
        Some(DialogCase::B1)
    );
    assert_eq!(
        dialog_case(0, Some(&l), &d).unwrap(),
        Some(DialogCase::B2 { m: 0x25 })
    );
    assert_eq!(
        dialog_case(0, Some(&list(1, 0, 0xFFFF)), &d).unwrap(),
        Some(DialogCase::Rest)
    );
    // PROVISIONAL: a longer list gives its first entry's string id; B1
    // does not need m; an empty list gives 0xFFFF.
    let l2 = list(2, 0, 0x25);
    assert_eq!(
        dialog_case(0, Some(&l2), &d).unwrap(),
        Some(DialogCase::B2 { m: 0x25 })
    );
    assert_eq!(list(0, 0, 0x25).first_m(), Some(0xFFFF));
    assert_eq!(
        dialog_case(0, Some(&l2), &npc_dialog(true)).unwrap(),
        Some(DialogCase::B1)
    );
}

// Covers: specs/client/msg-ui.md §16 r4
#[test]
fn npc_dialog_answer_is_kept_for_the_bridge() {
    use crate::bridge::msg::ui_npc::DialogCase;
    let w = world(false);
    let mut u = ui();
    let d = npc_dialog(false);
    // Without a 0x27 first: fatal 0x1060.
    assert!(matches!(
        u.apply_output(&Output::NpcDialog(Box::new(d.clone())), &w),
        Err(OriginalUiError::NoNpcText)
    ));
    u.apply_output(&npc_text(1, 1, 0, 0x25), &w).unwrap();
    u.take_outcome();
    u.apply_output(&Output::NpcDialog(Box::new(d.clone())), &w)
        .unwrap();
    assert_eq!(
        u.take_dialog_answer(),
        Some((Box::new(d.clone()), DialogCase::B2 { m: 0x25 }))
    );
    assert_eq!(u.take_dialog_answer(), None);
    assert_eq!(u.take_outcome().skipped, [skip::NPC_DIALOG_UI]);
    // PROVISIONAL: a 2-entry list gives its first entry's m.
    u.apply_output(&npc_text(1, 2, 0, 0x25), &w).unwrap();
    u.take_outcome();
    u.apply_output(&Output::NpcDialog(Box::new(d.clone())), &w)
        .unwrap();
    assert_eq!(
        u.take_dialog_answer(),
        Some((Box::new(d), DialogCase::B2 { m: 0x25 }))
    );
    assert_eq!(u.take_outcome().skipped, [skip::NPC_DIALOG_UI]);
}

// Covers: specs/ui/messages.md §6 r2
#[test]
fn npc_text_freed_at_interaction_end() {
    let w = world(false);
    let mut u = ui();
    u.apply_output(&npc_text(1, 1, 0, 0x25), &w).unwrap();
    assert!(u.npc_text().is_some());
    // The interaction ends (`0x004B3C20` → `0x004A1730`): freed.
    u.free_npc_text();
    assert_eq!(u.npc_text(), None);
    // Game exit (`0x004A0680`): freed too, and freeing none is harmless.
    u.apply_output(&npc_text(1, 1, 0, 0x25), &w).unwrap();
    u.free_npc_text();
    u.free_npc_text();
    assert_eq!(u.npc_text(), None);
}

// The edge cases of `ui/messages.md`, each reproduced.
// Covers: specs/ui/messages.md §edge-cases-original-bugs
#[test]
fn messages_edge_cases_reproduced() {
    use crate::ui::messages::chat::ScreenMessages;
    use crate::ui::messages::dialog::{DialogOpen, DialogUi, Scroll};
    use crate::ui::messages::hire::{HirePopup, StoneAnim};
    use crate::ui::messages::overhead::{
        BubbleBox, BubbleResult, OverheadPass, TimedBox, CAND_H, CAND_W, MOVE_TABLE,
    };
    use crate::ui::messages::{testutil::Fixed, Ltrb};
    let m = Fixed;
    let wide = |s: &str| s.encode_utf16().collect::<Vec<u16>>();
    // Timing runs on the wall clock in milliseconds, fed from the frame
    // clock: records, the timed box, the scroll and the stones take
    // millisecond ticks.
    let mut l = ScreenMessages::new();
    l.add(&wide("a"), 0, 5000, 800, false, &m);
    assert_eq!(l.records()[0].expiry, 15_000);
    assert_eq!(TimedBox::open(&wide("x"), 1000, &m).expiry, 1000 + 26 * 200);
    let mut sc = Scroll::new(4);
    sc.update(100);
    sc.update(108);
    assert_eq!(sc.p, (108 - 100) / 4 * 4);
    let mut st = StoneAnim::default();
    st.step(10);
    st.step(61);
    assert_eq!(st.counter, 1);
    // A refused bubble placement leaves font 13 current.
    let mut p = OverheadPass::default();
    p.begin((0, 0), None);
    let b = |x| BubbleBox::new(&wide("hello"), &m).at(x, 300);
    p.bubble(b(40), 100, 100, 0);
    assert_eq!(
        p.bubble(b(40), 100, 100, 0),
        BubbleResult::Refused {
            font_restored: false
        }
    );
    // Placement candidates are 400 × 280 and only diagonal.
    assert_eq!((CAND_W, CAND_H), (400, 280));
    assert_eq!(MOVE_TABLE, [1, 1, -1, 1, 1, -1, -1, -1]);
    let mut p = OverheadPass::default();
    p.begin((0, 0), None);
    p.place(Ltrb::new(40, 40, 140, 61), 800, 600);
    p.place(Ltrb::new(40, 40, 140, 61), 800, 600);
    assert_eq!(p.slots()[1], Ltrb::xywh(90, 75, 400, 280));
    // A bubble of more than 10 lines draws its first 10 lines at the
    // positions of the last 10.
    let mut big = BubbleBox {
        x: 0,
        y: 50,
        w: 50,
        h: 15 * 12 + 6,
        lines: (0..12).map(|i| wide(&i.to_string())).collect(),
        flag: 0,
    };
    let d = crate::ui::messages::overhead::bubble_draw(&mut big, 800, 600, 0).unwrap();
    assert_eq!((d.lines.len(), d.lines[0].y), (10, 50 + 18 + 30));
    // The timed box is centred on x = 320 at every resolution.
    let t = TimedBox::open(&wide("0123456789"), 0, &m);
    assert_eq!(t.bx.x, 320 - t.bx.w / 2);
    // A second dialog open keeps the old text.
    let mut d = DialogUi::default();
    d.open(
        DialogOpen::Npc { unit: None, id: 1 },
        &wide("5\nold"),
        800,
        600,
        0,
        0,
        1,
    );
    d.open(
        DialogOpen::Npc { unit: None, id: 2 },
        &wide("5\nnew"),
        800,
        600,
        0,
        0,
        2,
    );
    assert_eq!(d.panel.as_ref().unwrap().lines[0], wide("old"));
    // 0x27 with a type other than 1 or 2 frees the list without a null
    // test: with no list it is "no list" (not a crash).
    let w = world(false);
    let mut u = ui();
    u.apply_output(&npc_text(4, 1, 0, 9), &w).unwrap();
    assert_eq!(u.npc_text(), None);
    u.apply_output(&npc_text(1, 1, 0, 9), &w).unwrap();
    u.apply_output(&npc_text(0xFF, 1, 0, 9), &w).unwrap();
    assert_eq!(u.npc_text(), None);
    // 0x50 code 2 passes a hire-table entry the popup never reads: the
    // popup takes no entry.
    let mut h = HirePopup::default();
    h.open(None);
    assert!(h.popup);
    // The per-frame bubble box field +0x16 starts at 0 (drawn): d2rs uses
    // 0 (open question 2).
    assert_eq!(BubbleBox::new(&wide("x"), &m).flag, 0);
}
