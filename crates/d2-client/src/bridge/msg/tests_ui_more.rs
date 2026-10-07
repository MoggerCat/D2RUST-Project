// Spec: specs/client/msg-ui.md (§4–§22)
//! Test vectors of `client/msg-ui.md` §4–§22: "A" = `20261006-015956`,
//! "B" = `20261006-022633` recordings; the rest synthetic.

use super::super::output::Output;
use super::super::world::{
    ClientWorld, KindData, MonsterClass, PlayerData, SkillRow, UnitKey, ITEM, MONSTER, OBJECT,
    PLAYER,
};
use super::support::{hex, Model};

const P1: UnitKey = UnitKey::new(PLAYER, 1);

fn bytes(s: &str, len: usize) -> Vec<u8> {
    let mut b = hex(s);
    b.resize(len, 0);
    b
}

// Covers: specs/client/msg-ui.md §4 r1, §4 r2
#[test]
fn chat_captures_presence_and_player_name() {
    let mut m = Model::default();
    let k = UnitKey::new(MONSTER, 5);
    m.put(k);
    // `26 05 00 01 05000000 00 00 00 "hi" 00`: an empty name, text "hi".
    m.hex("26 05 00 01 05 00 00 00 00 00 00 68 69 00");
    let before = m.w.clone();
    assert_eq!(
        m.out,
        [Output::ChatLine {
            kind: 5,
            lang: 0,
            unit: k,
            b8: 0,
            b9: 0,
            name: vec![],
            text: b"hi".to_vec(),
            present: true,
            player_name: None,
        }]
    );
    assert_eq!(m.w, before, "no model change");
    // A player unit: its name is captured; absent units are not present.
    let mut name = [0u8; 16];
    name[..3].copy_from_slice(b"abc");
    m.put(P1).kind = KindData::Player(PlayerData {
        name,
        ..PlayerData::default()
    });
    m.out.clear();
    m.hex("26 04 00 00 01 00 00 00 00 00 6e 00 78 00");
    m.hex("26 04 00 02 09 00 00 00 00 00 00 78 00");
    let [Output::ChatLine {
        name: n,
        present: true,
        player_name: Some(p),
        ..
    }, Output::ChatLine {
        present: false,
        player_name: None,
        text,
        ..
    }] = &m.out[..]
    else {
        panic!("{:?}", m.out);
    };
    assert_eq!((n.as_slice(), p), (&b"n"[..], &name));
    assert_eq!(text, b"x");
}

// Covers: specs/client/msg-ui.md §5 r1
#[test]
fn npc_text_a37351() {
    let mut m = Model::default();
    m.put(UnitKey::new(MONSTER, 6));
    let msg = bytes("27 01 06 00 00 00 01 00 00 00 25 00", 40);
    m.recv(&msg);
    let mut b = [0u8; 40];
    b.copy_from_slice(&msg);
    assert_eq!(
        m.out,
        [Output::NpcText {
            bytes: b,
            present: true,
            object_class: 0
        }]
    );
    // Type 2: the object's class is captured.
    m.put(UnitKey::new(OBJECT, 6)).class = 77;
    m.out.clear();
    m.recv(&bytes("27 02 06 00 00 00 01", 40));
    assert!(matches!(
        m.out[..],
        [Output::NpcText {
            present: true,
            object_class: 77,
            ..
        }]
    ));
}

// Covers: specs/client/msg-ui.md §6 r1, §6 r2, §6 r3
#[test]
fn hire_offers_and_reset() {
    let mut m = Model::default();
    m.hex("4e 2a 00 78 56 34 12").hex("4f");
    assert_eq!(
        m.out,
        [
            Output::HireOffer {
                name: 0x2A,
                seed: 0x1234_5678
            },
            Output::HireListReset
        ]
    );
    assert_eq!(m.w, ClientWorld::default());
}

// Covers: specs/client/msg-ui.md §7 r1, §7 r2, §7 r3
#[test]
fn quest_special_codes() {
    let mut m = Model::default();
    m.hex("50 01 00 03 00 05 00 07 00 00 00 00 00 00 00");
    assert_eq!(
        m.out,
        [Output::QuestSpecial {
            code: 1,
            words: [3, 5, 7, 0, 0, 0]
        }]
    );
    // Code 23: C→S 0x69, `exit_requested`, and the output.
    m.out.clear();
    m.hex("50 17 00 00 00 00 00 00 00 00 00 00 00 00 00");
    assert_eq!(m.w.outgoing, [vec![0x69]]);
    assert!(m.w.exit_requested);
    assert!(matches!(m.out[..], [Output::QuestSpecial { code: 23, .. }]));
    // Code 6 (and any unlisted code): nothing, no output.
    let mut m = Model::default();
    m.hex("50 06 00 00 00 00 00 00 00 00 00 00 00 00 00");
    m.hex("50 05 00 00 00 00 00 00 00 00 00 00 00 00 00");
    assert!(m.out.is_empty() && m.w == ClientWorld::default());
}

// Covers: specs/client/msg-ui.md §8 r1, §8 r2, §8 r3
#[test]
fn open_ui_code_5_takes_the_cursor_item() {
    let mut m = Model::default();
    m.put(P1).kind = KindData::Player(PlayerData {
        cursor_item: Some(9),
        ..PlayerData::default()
    });
    m.w.local_player = Some(P1);
    m.put(UnitKey::new(ITEM, 9));
    m.hex("58 ff ff ff ff 05 01");
    let KindData::Player(p) = &m.unit(P1).kind else {
        panic!()
    };
    assert_eq!(p.cursor_item, None);
    assert!(m.w.units.contains_key(&UnitKey::new(ITEM, 9)));
    assert_eq!(
        m.out,
        [Output::OpenUi {
            guid: u32::MAX,
            code: 5,
            arg: 1
        }]
    );
    // Codes 2, 3 and > 7: fatal 0x354, no output.
    m.out.clear();
    for c in ["02", "03", "08"] {
        m.hex(&format!("58 ff ff ff ff {c} 00"));
    }
    assert!(m.out.is_empty());
    assert_eq!(
        m.rejected(),
        vec![(0x58, "fatal assert 0x354".to_string()); 3]
    );
}

// Covers: specs/client/msg-ui.md §9 r1, §9 r2
#[test]
fn npc_interact_b1563() {
    let mut m = Model::default();
    let k = UnitKey::new(MONSTER, 7);
    m.put(k).class = 148;
    m.hex("8a 01 07 00 00 00");
    let blocker = m.put(UnitKey::new(OBJECT, 3));
    blocker.class = 318;
    blocker.mode = 2;
    m.hex("8a 01 08 00 00 00");
    assert_eq!(
        m.out,
        [
            Output::NpcInteract {
                unit: k,
                present: true,
                class: 148,
                mdata_3c: None,
                blocker_open: false,
            },
            Output::NpcInteract {
                unit: UnitKey::new(MONSTER, 8),
                present: false,
                class: 0,
                mdata_3c: None,
                blocker_open: true,
            }
        ]
    );
}

// Covers: specs/client/msg-ui.md §10 r1, §11 r1, §12 r1, §13 r1, §14 r1, §15 r1
#[test]
fn record_outputs() {
    let mut m = Model::default();
    let mut gossip = hex("91 00 94 00");
    for _ in 0..11 {
        gossip.extend([0xFF, 0xFF]);
    }
    m.recv(&gossip);
    let mut trade = hex("78 61 62");
    trade.resize(17, 0);
    trade.extend([7, 0, 0, 0]);
    m.recv(&trade);
    m.recv(&bytes("29 01", 97));
    m.recv(&bytes("52 02", 42));
    m.recv(&bytes("5e 03", 38));
    m.hex("9b ff ff 00 00 00 00");
    let mut slots = [0xFFFF; 12];
    slots[0] = 148;
    let mut name = [0u8; 16];
    name[..2].copy_from_slice(b"ab");
    let mut record = [0u8; 96];
    record[0] = 1;
    let mut status = [0u8; 41];
    status[0] = 2;
    let mut avail = [0u8; 37];
    avail[0] = 3;
    assert_eq!(
        m.out,
        [
            Output::NpcIntro { slots },
            Output::TradePartner { name, guid: 7 },
            Output::GameQuestFlags { record },
            Output::QuestLog { status },
            Output::QuestAvailability { bytes: avail },
            Output::MercRevive {
                state: 0xFFFF,
                value: 0
            },
        ]
    );
    assert_eq!(m.w, ClientWorld::default());
}

/// `28 t guid r` + 96 bytes of quest flags.
fn quest_info(t: u8, guid: u32) -> Vec<u8> {
    let mut b = vec![0x28, t];
    b.extend(guid.to_le_bytes());
    b.push(0);
    b.push(1);
    b.resize(103, 0);
    b
}

// Covers: specs/client/msg-ui.md §16 r1, §16 r2, §16 r3
#[test]
fn quest_info_type_6_and_absent_npc() {
    let mut m = Model::default();
    m.recv(&quest_info(6, 0));
    let mut q = [0u8; 96];
    q[0] = 1;
    assert_eq!(m.out, [Output::QuestFlags { record: q }]);
    assert_eq!(m.w, ClientWorld::default(), "model unchanged");
    // T 1, unit (1, 6) absent: C→S 0x30 (u32 R = u8@6, u32 G), NpcGone.
    m.out.clear();
    m.recv(&quest_info(1, 6));
    assert_eq!(m.w.outgoing, [hex("30 00 00 00 00 06 00 00 00")]);
    assert_eq!(m.out, [Output::NpcGone { guid: 6 }]);
}

// Covers: specs/client/msg-ui.md §16 r4, §16 r5
#[test]
fn quest_info_npc_present_a37353() {
    let mut m = Model::default();
    let mut rows = vec![Some(MonsterClass::default()); 160];
    rows[148] = Some(MonsterClass {
        interact: true,
        npc: true,
        ..MonsterClass::default()
    });
    m.inputs.tables.monsters = rows;
    let k = UnitKey::new(MONSTER, 6);
    m.put(k).class = 148;
    m.put(UnitKey::new(MONSTER, 9)).class = 1;
    m.recv(&quest_info(1, 6));
    let u = m.unit(k);
    assert_eq!(u.flag_2, Some(true));
    assert_eq!(m.w.outgoing, [hex("2f 01 00 00 00 06 00 00 00")]);
    let [Output::NpcDialog(d)] = &m.out[..] else {
        panic!("{:?}", m.out)
    };
    assert_eq!((d.kind, d.guid, d.unit, d.class), (1, 6, k, 148));
    assert!(d.interact && !d.cursor_item);
    assert_eq!(d.npc_monsters, [k]);
    // Open question 10: the dialog branch's model writes and C→S 0x31
    // are not done by the bridge.
    assert_eq!(m.w.outgoing.len(), 1);
    assert_eq!(m.unit(k).mode, 0);
}

// Covers: specs/client/msg-ui.md §17 r1, §17 r2, §17 r3
#[test]
fn dialog_end() {
    let mut m = Model::default();
    let k = UnitKey::new(MONSTER, 6);
    m.put(k);
    m.hex("62 01 06 00 00 00 00");
    assert_eq!(m.unit(k).flag_2, Some(true));
    m.hex("62 03 06 00 00 00 00").hex("62 06 00 00 00 00 00");
    assert_eq!(
        m.out,
        [
            Output::NpcDialogEnd { kind: 1 },
            Output::NpcDialogEnd { kind: 6 }
        ]
    );
}

// Covers: specs/client/msg-ui.md §18 r1, §18 r2
#[test]
fn npc_transaction_a77950() {
    let mut m = Model::default();
    m.put(P1).stats.insert(14, 500);
    m.w.local_player = Some(P1);
    let msg = hex("2a 03 01 05 a4 f6 19 07 00 00 00 f4 01 00 00");
    m.recv(&msg);
    let mut b = [0u8; 15];
    b.copy_from_slice(&msg);
    assert_eq!(
        m.out,
        [Output::NpcTransaction {
            bytes: b,
            gold: 500
        }]
    );
}

// Covers: specs/client/msg-ui.md §19 r1, §19 r2, §19 r3
#[test]
fn event_text_cuts_the_name_and_code_0x12_sets_the_eclipse() {
    let mut m = Model::default();
    let mut long = hex("5a 02 04 00 00 00 00 00");
    long.extend(b"abcdefghijklmnopqrstuvwxyz0123");
    long.resize(40, 0);
    m.recv(&long);
    let [Output::EventText {
        bytes: cut,
        local_name,
    }] = &m.out[..]
    else {
        panic!()
    };
    assert_eq!(cut[0x17], 0, "cut to 15 characters");
    assert_eq!(cut[0x16], b'o');
    assert_eq!(*local_name, None);
    // Code 0x12 without a client act: no environment change.
    m.recv(&bytes("5a 12", 40));
    assert!(m.w.environment.is_none());
}

// Covers: specs/client/msg-ui.md §20 r1, §20 r2, §21 r1, §21 r2, §22 r1, §22 r2
#[test]
fn video_overhead_and_hotkey() {
    let mut m = Model::default();
    m.inputs.tables.skills = vec![SkillRow::default(); 5];
    m.hex("61 03").hex("76 00 01 00 00 00");
    // Skill 5 = the count passes (edge case); 6 is −1.
    m.hex("7b 02 05 80 07 00 00 00")
        .hex("7b 02 06 00 07 00 00 00");
    assert_eq!(
        m.out,
        [
            Output::ActVideo { video: 3 },
            Output::OverheadClear { unit: P1 },
            Output::HotkeyAssign {
                slot: 2,
                skill: 5,
                left: true,
                item: 7
            },
            Output::HotkeyAssign {
                slot: 2,
                skill: -1,
                left: false,
                item: 7
            },
        ]
    );
    assert_eq!(m.w.units.len(), 0);
}
