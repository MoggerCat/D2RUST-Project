//! An equipped item's stats reach the wearer (`item_link`; PROVISIONAL,
//! REC-161): attach on 0x1A, detach on 0x1C.

use super::*;

const STR: u16 = 0;

// Covers: specs/sim/stat-lists.md §8.4
#[test]
fn equipped_item_stats_reach_the_wearer_and_leave_with_it() {
    let mut w = World::new();
    w.state.link_item_stats = true;
    let c = w.cursor_item(CAP);
    let cu = w.unit(c).unwrap();
    w.set_stat(cu, STR, 5);
    let p = w.player;
    assert_eq!(w.stats.unit_total(p, STR, 0), 10);
    assert_eq!(w.handle(&body(0x1A, c, 1)), Ok(0));
    assert_eq!(w.mode(c), 1);
    assert_eq!(w.stats.unit_total(p, STR, 0), 15, "worn: +5 strength");
    assert_eq!(w.handle(&unequip(1)), Ok(0));
    assert_eq!(w.stats.unit_total(p, STR, 0), 10, "off again");
}

// Covers: specs/sim/stat-lists.md §8.4
#[test]
fn switch_off_leaves_the_player_alone() {
    let mut w = World::new();
    let c = w.cursor_item(CAP);
    let cu = w.unit(c).unwrap();
    w.set_stat(cu, STR, 5);
    assert_eq!(w.handle(&body(0x1A, c, 1)), Ok(0));
    assert_eq!(w.stats.unit_total(w.player, STR, 0), 10);
}

use crate::items::q;
use crate::items::tables::{PropRec, PropSlot, PropertyRec, SetItemRec, SetRec};

const DEX: u16 = 2;

/// A set of three with a cap (slot 0) and a sword (slot 1) in the
/// tables; the two-piece bonus is +5 dexterity (`properties.md` §11:
/// c = 2 → n = 2 → the first two partial records).
fn set_world() -> World {
    let mut w = World::new();
    w.state.link_item_stats = true;
    let mut p = PropertyRec::default();
    p.slots[0] = PropSlot {
        func: 1,
        stat: DEX,
        set: 0,
        val: 0,
    };
    w.tables.properties = vec![p];
    let mut partial = [PropRec::NONE; 8];
    partial[0] = PropRec {
        code: 0,
        param: 0,
        min: 5,
        max: 5,
    };
    w.tables.sets = vec![SetRec {
        count: 3,
        partial,
        full: [PropRec::NONE; 8],
    }];
    w.tables.setitems = (0..2)
        .map(|slot| SetItemRec {
            set: 0,
            slot,
            add_func: 2,
            props: [PropRec::NONE; 9],
            aprops: [PropRec::NONE; 10],
            ..Default::default()
        })
        .collect();
    w
}

fn set_piece(w: &mut World, record: usize, index: i32) -> Guid {
    let g = w.cursor_item(record);
    let u = w.unit(g).unwrap();
    let it = w.items.get_mut(u).unwrap();
    it.quality = q::SET;
    it.file_index = index;
    g
}

// Covers: specs/items/properties.md §11, §13
#[test]
fn set_bonus_applies_with_two_pieces_and_leaves_with_one() {
    let mut w = set_world();
    let p = w.player;
    let cap = set_piece(&mut w, CAP, 0);
    assert_eq!(w.handle(&body(0x1A, cap, 1)), Ok(0));
    assert_eq!(w.stats.unit_total(p, DEX, 0), 10, "one piece: no bonus");
    let sword = set_piece(&mut w, SWORD, 1);
    assert_eq!(w.handle(&body(0x1A, sword, 4)), Ok(0));
    assert!(w.state.errors.is_empty(), "{:?}", w.state.errors);
    assert_eq!(w.stats.unit_total(p, DEX, 0), 15, "two pieces: +5");
    assert_eq!(w.handle(&unequip(4)), Ok(0));
    assert_eq!(w.stats.unit_total(p, DEX, 0), 10, "bonus gone");
    assert!(w.state.errors.is_empty(), "{:?}", w.state.errors);
}
