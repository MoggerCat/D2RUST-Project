// Spec: specs/world/vendors.md §5.5, §7.1, §7.2, §8.1
//! Vendor ids through the host frame: the real trade functions on the
//! seam fake. Only the paths that end before a price is computed run
//! here; the priced paths (recorded buy 56, sell 500) are the vendors
//! module's own vectors (`d2_sim::world::vendors` tests), since their
//! tables have no provider outside `d2-sim`'s test fixtures.

use d2_sim::units::UnitId;
use d2_sim::world::npc;

use super::fake::*;
use super::*;

fn tx(code: u8, guid: u32, gold: u32) -> Vec<u8> {
    npc::transaction(0, code, guid, gold).to_vec()
}

/// A world with an item (GUID 7, mode 4 on the cursor) the player owns,
/// and one (GUID 8) it does not; Charsi is the interact unit when `talk`.
fn setup(talk: bool) -> (TestHost<FakeSim>, UnitId) {
    let (mut s, p) = sim();
    let w = &mut s.world.w;
    for (u, guid, owner) in [(UnitId(2007), 7, Some(p)), (UnitId(2008), 8, None)] {
        w.add(
            u,
            FUnit {
                guid,
                ty: 4,
                mode: 4,
                owner,
                ..FUnit::default()
            },
        );
    }
    if talk {
        w.interact.insert(p, (1, CHARSI));
    }
    (host(s), p)
}

fn assert_clean(h: &TestHost<FakeSim>) {
    assert!(h.game.world.faults.is_empty(), "{:?}", h.game.world.faults);
    assert!(h.game.unhandled.is_empty());
}

// Covers: specs/world/vendors.md §7.1 text, §7.1 r1
#[test]
fn buy_refusals_before_the_price() {
    let (mut h, p) = setup(false);
    // NPC not the interact unit → code 9, result 1.
    let (code, got) = send(&mut h, &hex("32 06000000 07000000 00000000 38000000"));
    assert_eq!(
        (code, got),
        (ResultCode::Refused, vec![tx(9, u32::MAX, 500)])
    );
    // Item missing → code 7 with the requested GUID, result 1.
    h.game.world.w.interact.insert(p, (1, CHARSI));
    let (code, got) = send(&mut h, &hex("32 06000000 99000000 00000000 38000000"));
    assert_eq!((code, got), (ResultCode::Refused, vec![tx(7, 0x99, 500)]));
    // Not in the store (t = 0) → code 7, GUID −1 (TODO in the module).
    let (code, got) = send(&mut h, &hex("32 06000000 08000000 00000000 38000000"));
    assert_eq!(
        (code, got),
        (ResultCode::Refused, vec![tx(7, u32::MAX, 500)])
    );
    assert_clean(&h);
}

// Covers: specs/world/vendors.md §7.2 r1, §7.2 r2, §7.2 r3, §7.2 r4
#[test]
fn sell_refusals_before_the_price() {
    let (mut h, p) = setup(false);
    // Item missing → result 1, no message.
    let (code, got) = send(&mut h, &hex("33 06000000 99000000 0400 0000 f4010000"));
    assert_eq!((code, got), (ResultCode::Refused, vec![]));
    // NPC not the interact unit → code 9, result 1.
    let (code, got) = send(&mut h, &hex("33 06000000 07000000 0400 0000 f4010000"));
    assert_eq!(
        (code, got),
        (ResultCode::Refused, vec![tx(9, u32::MAX, 500)])
    );
    h.game.world.w.interact.insert(p, (1, CHARSI));
    // Not the player's item → code 11, result 3.
    let (code, got) = send(&mut h, &hex("33 06000000 08000000 0400 0000 f4010000"));
    assert_eq!(
        (code, got),
        (ResultCode::Malformed, vec![tx(11, u32::MAX, 500)])
    );
    // Item mode ≠ u16 @9 → code 9, result 3.
    let (code, got) = send(&mut h, &hex("33 06000000 07000000 0000 0000 f4010000"));
    assert_eq!(
        (code, got),
        (ResultCode::Malformed, vec![tx(9, u32::MAX, 500)])
    );
    assert_clean(&h);
}

// Covers: specs/world/vendors.md §8.1 r1, §8.1 r2
#[test]
fn repair_refusals() {
    let (mut h, p) = setup(false);
    // NPC not the interact unit → code 9 (result 0, module TODO V12).
    let (code, got) = send(&mut h, &hex("35 06000000 07000000 0000 0000 00000000"));
    assert_eq!((code, got), (ResultCode::Done, vec![tx(9, u32::MAX, 500)]));
    // Akara does not repair → code 9.
    h.game.world.w.interact.insert(p, (1, AKARA));
    let (code, got) = send(&mut h, &hex("35 10000000 07000000 0000 0000 00000000"));
    assert_eq!((code, got), (ResultCode::Done, vec![tx(9, u32::MAX, 500)]));
    assert_clean(&h);
}

// Covers: specs/world/vendors.md §5.5
#[test]
fn identify_gamble() {
    let (mut h, _) = setup(true);
    // Unknown item → 2; not the last bought item → 3.
    let (code, _) = send(&mut h, &hex("37 99000000"));
    assert_eq!(code, ResultCode::Invalid);
    let (code, _) = send(&mut h, &hex("37 07000000"));
    assert_eq!(code, ResultCode::Malformed);
    // The last bought item: identified, result 0, no message (a fresh
    // client: the transport filters a repeated message, §2.1).
    let (mut h, p) = setup(true);
    h.game.world.w.last_bought.insert(p, 7);
    let (code, got) = send(&mut h, &hex("37 07000000"));
    assert_eq!((code, got), (ResultCode::Done, vec![]));
    assert_eq!(h.game.world.w.units[&UnitId(2007)].flags & 0x10, 0x10);
    assert_clean(&h);
}
