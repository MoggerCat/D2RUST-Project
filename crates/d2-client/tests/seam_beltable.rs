// Spec: specs/seams/item-grids.md (§2.8)
//! The belt test is one fact on both sides (`seams/item-grids.md` §2.8):
//! the client's `fits_belt` (Shift-click, belt click, cursor highlight)
//! equals the sim's `beltable` (`0x0062BAD0`, itemtypes `beltable`) on
//! every items row of the user's tables.

mod app_support;

use d2_client::ui::panels::inv_items::fits_belt;
use d2_sim::items::inventory::{belt::beltable, InvTables};

// Covers: specs/seams/item-grids.md §2.8
#[test]
#[ignore = "real data: needs D2_GAME_DIR (tools/realdata-gate.sh)"]
fn the_client_belt_test_is_the_sim_beltable_on_the_install() {
    let d = app_support::live();
    let t = InvTables::from_fixed(&d.tables.fixed).expect("the inventory tables load");
    assert!(!t.items.is_empty());
    for (i, r) in t.items.iter().enumerate() {
        // The first row of a code is the one the client finds.
        if t.items[..i].iter().any(|p| p.code == r.code) {
            continue;
        }
        assert_eq!(
            fits_belt(Some(&t), Some(r.code)),
            beltable(&t, i),
            "{}",
            String::from_utf8_lossy(&r.code)
        );
    }
    // The scrolls the old fixed code list left out (q-seam-audit
    // F-items-1) go to the belt; a helm does not.
    for code in [b"hp1 ", b"isc ", b"tsc "] {
        assert!(
            fits_belt(Some(&t), Some(*code)),
            "{}",
            String::from_utf8_lossy(code)
        );
    }
    assert!(!fits_belt(Some(&t), Some(*b"cap ")));
}
