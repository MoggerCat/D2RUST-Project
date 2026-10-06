// Spec: specs/world/quests.md §1.7, §6.2, §7.3
//! Quest ids through the host frame: the real `QuestControl` on the seam
//! fake; the chat end goes through the NPC module's 0x30 (`npc.md` §3).

use super::fake::*;
use super::*;

// Covers: specs/world/quests.md §7.3
#[test]
fn akara_start_message_then_chat_end() {
    // `015956` frames 1729–1751: 0x31 (Akara, message 64) → 0x27, 0x29;
    // the chat end → `5d 01 00 01 0000`; slot 1 = `04 00`.
    let (s, p) = sim();
    let mut h = host(s);
    let (code, got) = send(&mut h, &hex("31 10000000 4000 0000"));
    assert_eq!(code, ResultCode::Done);
    let ids: Vec<u8> = got.iter().map(|m| m[0]).collect();
    assert_eq!(ids, [0x27, 0x29]);
    let (code, got) = send(&mut h, &hex("30 01000000 10000000"));
    assert_eq!(
        (code, got),
        (ResultCode::Done, vec![hex("5d 01 00 01 0000")])
    );
    assert_eq!(h.game.world.w.quests[&p].flags[0].0[2..4], [0x04, 0x00]);
    assert!(h.game.world.faults.is_empty());
}

// Covers: specs/world/quests.md §6.2 r1, §6.2 r2, §6.2 r4
#[test]
fn request_quest_data() {
    let (s, p) = sim();
    let mut h = host(s);
    let (code, got) = send(&mut h, &[0x40]);
    assert_eq!(code, ResultCode::Done);
    let record = h.game.world.w.quests[&p].flags[0].0;
    assert_eq!(
        got,
        vec![
            [&hex("28 06 00000000 00")[..], &record[..]].concat(),
            [&[0x52u8][..], &[0u8; 41][..]].concat(),
        ]
    );
    assert!(h.game.world.faults.is_empty());
}

// Covers: specs/world/quests.md §1.7
#[test]
fn quest_completed_sets_the_log_bit() {
    let (s, p) = sim();
    let mut h = host(s);
    let (code, got) = send(&mut h, &hex("58 0500"));
    assert_eq!((code, got), (ResultCode::Done, vec![]));
    assert!(h.game.world.w.quests[&p].flags[0].get(5, 12));
    let (code, got) = send(&mut h, &hex("58 2a00"));
    assert_eq!((code, got), (ResultCode::Invalid, vec![]));
    assert!(h.game.world.faults.is_empty());
}
