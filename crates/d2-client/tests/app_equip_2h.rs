// Spec: specs/items/inventory.md (§4.3, §4.6)
//! A new sorceress takes her two-handed start staff off the right hand
//! (C→S 0x1C) and puts it straight back (C→S 0x1A) on the user's install
//! (q-fix-real-equip-2h).

mod app_support;
mod real_rig;

use d2_client::bridge::BridgeResource;
use real_rig::Rig;

const RIGHT: u8 = 4;

// Covers: specs/items/inventory.md §4.3
// Covers: specs/items/inventory.md §4.6
#[test]
#[ignore = "real data: needs D2_GAME_DIR (tools/realdata-gate.sh)"]
fn the_two_handed_start_staff_goes_back_on_the_right_hand() {
    let mut r = Rig::new("sorceress", &[]);
    let staff = r.worn(RIGHT).expect("the start staff in the right hand");
    let cursor = |r: &Rig| {
        let w = r.app.world().resource::<BridgeResource>().0.world();
        d2_client::bridge::items::cursor_item(w).map(|i| i.key.guid)
    };
    let fate = r.send_for_fate(&d2_proto::client::RemoveBodyItem { bodyloc: RIGHT }.encode());
    r.step(6);
    let guid = cursor(&r).unwrap_or_else(|| panic!("the staff on the cursor ({fate})"));
    assert_eq!(r.worn(RIGHT), None);
    let msg = d2_client::bridge::items::equip(guid, RIGHT);
    let bytes = msg.encode();
    let fate = r.send_for_fate(&bytes);
    r.step(6);
    assert_eq!(r.worn(RIGHT), Some(staff), "worn again (0x1A: {fate})");
    assert_eq!(cursor(&r), None, "the cursor is empty");
}
