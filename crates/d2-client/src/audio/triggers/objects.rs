// Spec: specs/audio/triggers.md §7 (object mode sounds)
// Spec: specs/audio/object-sounds.tsv
//! Object mode sounds (`0x004CB460` on objects): Cain's gibbet line, the
//! mode loop `0x004CB3E0` and the transition `0x004CB380`.

use super::tables::{ObjectSoundRow, ObjectSounds, MAX_OBJECT_CLASS};
use super::{Ctx, TriggerError, Unit, UnitSound};

/// Class 26 (Cain's gibbet), its line 3,671 `cain_act1_help` and the
/// distance below which it plays (§7 r2).
pub const CAIN_GIBBET: i32 = 26;
pub const CAIN_HELP: i32 = 3671;
pub const CAIN_DIST: i32 = 20;
/// Class 59 (town portal): transitions also on the first call (§7 r5).
pub const TOWN_PORTAL: i32 = 59;
/// Class 61: cairn stone loops by mode (§7 r4, table `0x00728338`).
pub const CAIRN: i32 = 61;
/// The cairn loop table `0x00728338` by mode (§7 r8): modes 1–5 → 413–417
/// (`cairn_stone_1` … `cairn_stone_5`), modes 0, 6, 7 → 0.
pub const CAIRN_LOOPS: [i32; 8] = [0, 413, 414, 415, 416, 417, 0, 0];
/// Loop mode value meaning "any mode" (§7 TSV).
pub const ANY_MODE: u8 = 8;

/// Object mode sound call on object U in its current mode (§7 r1–r6).
/// A class without a TSV row (120 of 573) behaves as an all-zero row
/// (§7 r7): no transition, loop 0 (U is detached from its looping
/// requests), and U+0x74 / U+0x70 still updated. Class ≥ 573 or mode ≥ 8
/// is fatal. Class 61 loops the cairn table by mode (§7 r8).
pub fn object_mode(
    cx: &mut Ctx,
    table: &ObjectSounds,
    u: &Unit,
    us: &mut UnitSound,
) -> Result<(), TriggerError> {
    let c = u.class;
    let m = u.mode;
    if !(0..=MAX_OBJECT_CLASS).contains(&c) {
        return Err(TriggerError::ObjectClass(c));
    }
    if m >= 8 {
        return Err(TriggerError::ObjectMode(m));
    }
    let zero = ObjectSoundRow {
        class: c,
        modes: [0; 8],
        loop_a: 0,
        loop_a_mode: 0,
        loop_b: 0,
        loop_b_mode: 0,
        ordered: false,
    };
    let has_record = table.get(c).is_some();
    let row = table.get(c).unwrap_or(&zero);
    // r2.
    if c == CAIN_GIBBET && m == 0 && us.last_idle == 0 && u.local_dist < CAIN_DIST {
        cx.req(CAIN_HELP, Some(u.key), 0);
        us.last_idle = cx.c;
    }
    // r3.
    if us.obj_seen && m == us.obj_prev_mode {
        return Ok(());
    }
    // r4 (`0x004CB3E0`; a null record gives 0, r7).
    let s = if !has_record {
        0
    } else if c == CAIRN {
        CAIRN_LOOPS[m as usize]
    } else if row.loop_a_mode == m || row.loop_a_mode == ANY_MODE {
        row.loop_a
    } else if row.loop_b_mode == m || row.loop_b_mode == ANY_MODE {
        row.loop_b
    } else {
        0
    };
    let reqs = cx.s.unit_requests(u.key);
    if s != 0 {
        let base = cx.s.group_base(s);
        if !reqs.iter().any(|&(_, id)| cx.s.group_base(id) == base) {
            cx.req(s, Some(u.key), 0);
        }
    } else {
        for (h, id) in reqs {
            if cx.s.looping(id) {
                cx.s.detach(h, u.key, false);
            }
        }
    }
    // r5.
    if us.obj_seen || c == TOWN_PORTAL {
        let mut t = row.modes.get(m as usize).copied().unwrap_or(0);
        if row.ordered && m < us.obj_prev_mode {
            t = 0;
        }
        if t != 0 {
            cx.req(t, Some(u.key), 0);
        }
    }
    // r6.
    us.obj_prev_mode = m;
    us.obj_seen = true;
    Ok(())
}
