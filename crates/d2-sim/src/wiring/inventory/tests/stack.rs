//! 0x21 StackItems (§7.12) and 0x22 UnstackItems (§7.13) on real stats.

use super::*;
use crate::items::moves::stat::{DURABILITY, QUANTITY};

/// `dst` stored in page 0 with quantity `qd`, `src` on the cursor with
/// quantity `qs`; drained.
fn pair(w: &mut World, record: usize, qd: i32, qs: i32) -> (Guid, Guid) {
    let dst = w.cursor_item(record);
    assert_eq!(w.handle(&insert(dst, 0, 0, 0)), Ok(0));
    w.drain();
    let src = w.cursor_item(record);
    for (g, q) in [(dst, qd), (src, qs)] {
        let u = w.unit(g).unwrap();
        w.set_stat(u, QUANTITY, q);
    }
    (dst, src)
}

fn qty(w: &World, g: Guid) -> i32 {
    w.stats.unit_total(w.unit(g).unwrap(), QUANTITY, 0)
}

/// §7.12, sum over the max stack (keys: `maxstack` 12 + stat 254 0): dst
/// := 12, src := the rest, each announced (0x3E, seam), dst item flag
/// 0x8, command flag 0x100 → 0x9C action 0xA (row 9). Both stay.
#[test]
fn stack_over_the_max_fills_dst() {
    let mut w = World::new();
    let (dst, src) = pair(&mut w, KEY, 8, 7);
    w.rest.log.clear();
    assert_eq!(w.handle(&msg(0x21, &[src, dst])), Ok(0));
    assert_eq!((qty(&w, dst), qty(&w, src)), (12, 3));
    assert_eq!(
        w.rest.log,
        [
            format!("send_item_stat {dst} 70"),
            format!("send_item_stat {src} 70"),
        ]
    );
    assert_eq!(w.data(dst).flags & 0x8, 0x8);
    assert_eq!(w.inventory().cursor(), w.unit(src));
    assert_eq!(item_msgs(&w.drain()), [(0x9C, 0x0A, dst)]);
}

/// §7.12 merge branch: throwing knives have durability (`0x00629930`:
/// durability 20, a stat list, stat 152 < 1): src's lower stat 72 lowers
/// dst's (OQ15), dst := q_s + q_d, cursor cleared, S→C 0x42 for src, src
/// freed (its unit is gone); dst 0x9C action 0xA.
#[test]
fn stack_merge_frees_the_source() {
    let mut w = World::new();
    let (dst, src) = pair(&mut w, KNIFE, 10, 5);
    let (du, su) = (w.unit(dst).unwrap(), w.unit(src).unwrap());
    w.set_stat(du, DURABILITY, 20);
    w.set_stat(su, DURABILITY, 7);
    assert_eq!(w.handle(&msg(0x21, &[src, dst])), Ok(0));
    assert_eq!(qty(&w, dst), 15);
    assert_eq!(w.stats.unit_total(du, DURABILITY, 0), 7);
    assert_eq!(w.inventory().cursor(), None);
    assert_eq!(w.unit(src), None, "freed");
    assert!(!w.state.items.contains_key(&su));
    let mut clear = vec![0x42, 4];
    clear.extend_from_slice(&src.to_le_bytes());
    assert_eq!(w.rest.sent, [(w.me(), clear)]);
    assert_eq!(item_msgs(&w.drain()), [(0x9C, 0x0A, dst)]);
    assert!(w.state.errors.is_empty());
}

/// §7.12: keys have no durability → no merge (both quantities stay), dst
/// is still marked (0x9C action 0xA). src = dst → 3; different classes
/// fail §4.5 → 0. §7.13: 0x22 on an owned item → 3 (X1), on another → 1.
#[test]
fn stack_without_merge_and_unstack() {
    let mut w = World::new();
    let (dst, src) = pair(&mut w, KEY, 3, 4);
    assert_eq!(w.handle(&msg(0x21, &[src, src])), Ok(3));
    assert_eq!(w.handle(&msg(0x21, &[src, dst])), Ok(0));
    assert_eq!((qty(&w, dst), qty(&w, src)), (3, 4));
    assert_eq!(item_msgs(&w.drain()), [(0x9C, 0x0A, dst)]);
    let other = w.ground_item(KNIFE, 12, 12);
    assert_eq!(w.handle(&msg(0x21, &[src, other])), Ok(1), "not owned");
    assert_eq!(w.handle(&msg(0x22, &[dst])), Ok(3));
    assert_eq!(w.handle(&msg(0x22, &[other])), Ok(1));
}
