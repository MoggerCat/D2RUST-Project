// Spec: specs/sim/intents-events.md (Test vectors; §2.4 rules 3, 7, 9, 10)
//! Typed decode/encode of the fixed layouts, from the spec's vectors.

use d2_proto::client::{BindHotkey, CreateGame, EquipItem, SelectSkill, Walk, WalkToUnit};
use d2_proto::schema::{FieldType, Gate, HandlerSize, Kind, Scope, SizeRule};
use d2_proto::server::{LoadAct, SetStatWord};
use d2_proto::{DecodeError, FixedMessage, CLIENT_MESSAGES, SERVER_MESSAGES};

#[test]
fn select_skill_vector() {
    let b = [0x3C, 0x05, 0x00, 0x00, 0x80, 0xFF, 0xFF, 0xFF, 0xFF];
    let m = SelectSkill::decode(&b).unwrap();
    assert_eq!(
        m,
        SelectSkill {
            skill: 5,
            left: true,
            item: u32::MAX
        }
    );
    assert_eq!(m.encode(), b);
}

#[test]
fn bind_hotkey_vector() {
    let b = [0x51, 0x06, 0x80, 0x03, 0x00, 0xFF, 0xFF, 0xFF, 0xFF];
    let m = BindHotkey::decode(&b).unwrap();
    assert_eq!(
        m,
        BindHotkey {
            skill: 6,
            left: true,
            slot: 3,
            item: u32::MAX
        }
    );
    assert_eq!(m.encode(), b);
}

#[test]
fn walk_and_unit() {
    let b = [0x01, 0x10, 0x00, 0x20, 0x00];
    assert_eq!(Walk::decode(&b).unwrap(), Walk { x: 0x10, y: 0x20 });
    assert_eq!(Walk { x: 0x10, y: 0x20 }.encode(), b);
    let u = WalkToUnit {
        type_: 1,
        id: 0x1234_5678,
    };
    assert_eq!(u.encode(), [0x02, 1, 0, 0, 0, 0x78, 0x56, 0x34, 0x12]);
    assert_eq!(WalkToUnit::decode(&u.encode()).unwrap(), u);
}

#[test]
fn decode_errors() {
    assert_eq!(Walk::decode(&[]), Err(DecodeError::Empty));
    assert_eq!(
        Walk::decode(&[0x03, 0, 0, 0, 0]),
        Err(DecodeError::WrongId {
            expected: 0x01,
            found: 0x03
        })
    );
    // Handler 0x01 with size 6: wrong size (§2.4 rule 1).
    assert_eq!(
        Walk::decode(&[0x01, 0, 0, 0, 0, 0]),
        Err(DecodeError::WrongSize {
            expected: 5,
            found: 6
        })
    );
}

/// Body location is a u8 inside a 4-byte slot; bytes +6..+8 are ignored
/// (§2.4 rule 9) and encode as 0.
#[test]
fn body_location_u8() {
    let b = [0x1A, 7, 0, 0, 0, 4, 0xAA, 0xBB, 0xCC];
    let m = EquipItem::decode(&b).unwrap();
    assert_eq!(
        m,
        EquipItem {
            item: 7,
            bodyloc: 4
        }
    );
    assert_eq!(m.encode(), [0x1A, 7, 0, 0, 0, 4, 0, 0, 0]);
}

#[test]
#[should_panic(expected = "does not fit")]
fn bit_field_overflow_panics() {
    let _ = BindHotkey {
        skill: 0x8000,
        left: false,
        slot: 0,
        item: 0,
    }
    .encode();
}

#[test]
fn session_and_server_layouts() {
    let mut name = [0u8; 16];
    name[..3].copy_from_slice(b"Bob");
    let m = CreateGame {
        char_name: name,
        unk_43: 1,
        unk_44: 2,
        locale: 3,
        ..Default::default()
    };
    let b = m.encode();
    assert_eq!(b.len(), 46);
    assert_eq!(&b[0x15..0x18], b"Bob");
    assert_eq!(&b[0x2B..], &[1, 2, 3]);
    assert_eq!(CreateGame::decode(&b).unwrap(), m);

    let s = SetStatWord {
        stat: 12,
        value: 0x0102,
    };
    assert_eq!(s.encode(), [0x1E, 12, 0x02, 0x01]);
    assert_eq!(LoadAct::SIZE, 12);
    assert_eq!(LoadAct::decode(&[0x03; 12]).unwrap().act, 3);
}

/// Descriptor spot checks against the spec's prose (§2.1, §2.3, §2.4).
#[test]
fn descriptors() {
    assert_eq!(CLIENT_MESSAGES.len(), 0x71);
    assert_eq!(SERVER_MESSAGES.len(), 0xB5);
    for (i, m) in CLIENT_MESSAGES.iter().enumerate() {
        assert_eq!(m.id as usize, i);
        // Transport and handler sizes agree for every id (§2.4 rule 1).
        if let HandlerSize::Exact(n) = m.handler_size {
            assert_eq!(m.transport_size, SizeRule::Fixed(n));
        }
    }
    for (i, m) in SERVER_MESSAGES.iter().enumerate() {
        assert_eq!(m.id as usize, i);
    }
    // Ids never queued (§2.1 rule 5).
    let never: Vec<u8> = CLIENT_MESSAGES
        .iter()
        .filter(|m| m.transport_size.is_never())
        .map(|m| m.id)
        .collect();
    assert_eq!(
        never,
        [0x00, 0x2B, 0x2C, 0x4A, 0x4E, 0x55, 0x56, 0x57, 0x5A, 0x5B, 0x5C, 0x64, 0x65, 0x6F]
    );
    // Gates (§2.3 rule 3).
    let ungated: Vec<u8> = CLIENT_MESSAGES
        .iter()
        .filter(|m| m.gate == Gate::None)
        .map(|m| m.id)
        .collect();
    assert_eq!(ungated, [0x14, 0x15, 0x3C, 0x43, 0x66]);
    assert_eq!(CLIENT_MESSAGES[0x41].gate, Gate::Dead);
    // Stubs (§2.4 rule 2).
    let stub = |k| -> Vec<u8> {
        CLIENT_MESSAGES
            .iter()
            .filter(|m| m.kind == k)
            .map(|m| m.id)
            .collect()
    };
    assert_eq!(stub(Kind::Stub3), [0x2C, 0x2D, 0x39, 0x45, 0x52]);
    assert_eq!(stub(Kind::Stub0), [0x2E, 0x42, 0x43, 0x66]);
    // 80 intents in scope (§4 rule 1).
    let sim = CLIENT_MESSAGES
        .iter()
        .filter(|m| m.scope == Scope::Sim)
        .count();
    assert_eq!(sim, 80);
    assert_eq!(CLIENT_MESSAGES[0x3C].layout[1].ty, FieldType::Bit(31));
    // S→C size-0 ids with builders (§3.1 rule 3).
    for id in [0x83, 0x84, 0x88] {
        let m = &SERVER_MESSAGES[id];
        assert!(m.size.is_never() && !m.senders.is_empty());
    }
}
