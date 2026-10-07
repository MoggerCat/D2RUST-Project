// Spec: specs/client/msg-units.md (§6), specs/client/stat-lists.md (§3)
//! Unit states: S→C 0xA7 DelayedState, 0xA8 SetState, 0xA9 EndState
//! (`client/stat-lists.md` §3 rules 1–4) and 0xAA AddUnit
//! (`msg-units.md` §6 rule 2), over the state bits and state lists of
//! `ClientUnit` ([`state_on`], [`state_stat`], [`state_off`]).
//!
//! The overlay, light and animation refreshes of state on / off and the
//! state on hooks `0x004D9E60` (the states `setfunc`, a state missile)
//! are client presentation (`client/stat-lists.md` open question 6,
//! Phase 6): no model field changes.

use super::super::bits::BitReader;
use super::super::dispatch::{HandlerError, Message};
use super::super::world::{ClientWorld, ModelInputs, UnitKey, MONSTER, PLAYER};
use super::Bytes;

/// Dead for the state flag `[0x006CE284]` (`client/stat-lists.md` §3
/// r3): monster mode 12, player mode 17.
fn dead_for_states(w: &ClientWorld, key: UnitKey) -> bool {
    w.units.get(&key).is_some_and(|u| {
        (u.key.unit_type == MONSTER && u.mode == 12) || (u.key.unit_type == PLAYER && u.mode == 17)
    })
}

/// State on `0x004D9B20(unit, state)` (`client/stat-lists.md` §3 r3). A
/// flag of the states row is read only when it decides the outcome; a
/// missing row then is a handler error (the tables are an input).
pub fn state_on(
    w: &mut ClientWorld,
    inputs: &ModelInputs,
    key: UnitKey,
    state: u8,
) -> Result<(), HandlerError> {
    let row = inputs.tables.states.get(usize::from(state)).copied();
    let missing =
        || HandlerError::Invalid("client/stat-lists.md §3 r3: no states row for the state's flags");
    if dead_for_states(w, key) && row.ok_or_else(missing)?.dead_bit_only {
        if let Some(u) = w.units.get_mut(&key) {
            u.states.insert(state);
        }
        return Ok(());
    }
    let Some(u) = w.units.get_mut(&key) else {
        return Ok(());
    };
    if u.state_lists.contains_key(&state) && !row.ok_or_else(missing)?.keep_list {
        u.state_lists.remove(&state);
    }
    u.states.insert(state);
    Ok(())
}

/// State off `0x00639DB0(unit, state, 0)`: the bit only.
pub fn state_bit_off(w: &mut ClientWorld, key: UnitKey, state: u8) {
    if let Some(u) = w.units.get_mut(&key) {
        u.states.remove(&state);
    }
}

/// State off with its list (0xA9: `0x004D9F40` then `0x004D9C30`; 0x7C:
/// `0x00639DB0`, `0x006277E0`, `0x00626CD0`): the bit cleared, the
/// state's list detached and freed.
pub fn state_off(w: &mut ClientWorld, key: UnitKey, state: u8) {
    if let Some(u) = w.units.get_mut(&key) {
        u.states.remove(&state);
        u.state_lists.remove(&state);
    }
}

/// The state stat `0x004D9D70(unit, list, state, stat, value, param)`
/// (`client/stat-lists.md` §3 r2): the unit's list of the state (made
/// and attached when it has none), then the set `0x00627150`
/// (`sim/stat-lists.md` §5 r1: a value 0 removes the entry).
/// TODO(spec: client/stat-lists.md §3 r2): stat 172's `0x00463C00(value)`
/// and the stats refresh `0x00623F50` change no model field the specs
/// name.
pub fn state_stat(w: &mut ClientWorld, key: UnitKey, state: u8, stat: u16, param: u16, value: i32) {
    if let Some(u) = w.units.get_mut(&key) {
        let list = u.state_lists.entry(state).or_default();
        if value == 0 {
            list.remove(&(stat, param));
        } else {
            list.insert((stat, param), value);
        }
    }
}

/// Why a state stat stream stopped.
enum StatsEnd {
    /// 0x1FF: the state's hooks run.
    Done,
    /// A stat outside the table or with `Send Bits` 0.
    Abort,
}

/// One state's stat stream (0xA8 rule 1, 0xAA rule 2.3): stat id 9 bits,
/// param, value; each added to the state's list.
fn read_stats(
    w: &mut ClientWorld,
    inputs: &ModelInputs,
    key: UnitKey,
    state: u8,
    r: &mut BitReader<'_>,
) -> StatsEnd {
    loop {
        let id = r.read(9);
        if id == 0x1FF {
            return StatsEnd::Done;
        }
        let Some(row) = inputs.tables.stats.get(id as usize) else {
            return StatsEnd::Abort;
        };
        if row.bits == 0 {
            return StatsEnd::Abort;
        }
        let param = if row.param_bits > 0 {
            r.read_signed(u32::from(row.param_bits)) as u16
        } else {
            0
        };
        let bits = u32::from(row.bits);
        let value = if bits < 32 && row.signed {
            r.read_signed(bits)
        } else {
            r.read(bits) as i32
        };
        state_stat(w, key, state, id as u16, param, value);
    }
}

/// 0xA7 DelayedState, 0xA9 EndState (`client/stat-lists.md` §3 r4): type
/// u8@1, GUID u32@2, state u8@6; a unit in S → 0xA7 state on (then the
/// hooks), 0xA9 state off.
pub fn delayed_or_end_state(w: &mut ClientWorld, msg: &Message<'_>) -> Result<(), HandlerError> {
    let b = Bytes(msg.bytes);
    if msg.bytes.len() != 7 {
        return Err(HandlerError::Invalid("0xA7 / 0xA9 are 7 bytes"));
    }
    let key = UnitKey::new(b.u8(1)?, b.u32(2)?);
    let state = b.u8(6)?;
    if !w.units.contains_key(&key) {
        return Ok(());
    }
    if msg.id == 0xA7 {
        state_on(w, msg.inputs, key, state)
    } else {
        state_off(w, key, state);
        Ok(())
    }
}

/// 0xA8 SetState (`client/stat-lists.md` §3 r1): type u8@1, GUID u32@2,
/// size u8@6, state u8@7, the stat stream from @8.
pub fn set_state(w: &mut ClientWorld, msg: &Message<'_>) -> Result<(), HandlerError> {
    let b = Bytes(msg.bytes);
    let size = usize::from(b.u8(6)?);
    if size != msg.bytes.len() || size < 8 {
        return Err(HandlerError::Invalid("0xA8 size byte"));
    }
    let key = UnitKey::new(b.u8(1)?, b.u32(2)?);
    let state = b.u8(7)?;
    if !w.units.contains_key(&key) {
        return Ok(());
    }
    state_on(w, msg.inputs, key, state)?;
    let mut r = BitReader::new(&msg.bytes[8..]);
    // Either end runs the hooks (rule 1: "after the stream").
    let _ = read_stats(w, msg.inputs, key, state, &mut r);
    Ok(())
}

/// 0xAA AddUnit (`msg-units.md` §6 rules 2–3): type u8@1, GUID u32@2,
/// size u8@6, the state stream from @7.
pub fn add_unit_states(w: &mut ClientWorld, msg: &Message<'_>) -> Result<(), HandlerError> {
    let b = Bytes(msg.bytes);
    let size = usize::from(b.u8(6)?);
    if size != msg.bytes.len() || size < 7 {
        return Err(HandlerError::Invalid("0xAA size byte"));
    }
    let key = UnitKey::new(b.u8(1)?, b.u32(2)?);
    if !w.units.contains_key(&key) {
        return Ok(());
    }
    let mut r = BitReader::new(&msg.bytes[7..]);
    loop {
        let s = r.read(8);
        if r.overflow {
            // Rule 3 (d2rs): a stream without its closing 0xFF.
            return Err(HandlerError::Invalid(
                "0xAA: the state stream ran past its end (no closing 0xFF)",
            ));
        }
        if s >= 255 {
            return Ok(());
        }
        let state = s as u8;
        state_on(w, msg.inputs, key, state)?;
        if r.read(1) == 1 {
            if let StatsEnd::Abort = read_stats(w, msg.inputs, key, state, &mut r) {
                // Rule 2.3: the whole message ends, no hooks.
                return Ok(());
            }
        }
        if r.overflow {
            return Err(HandlerError::Invalid(
                "0xAA: the state stream ran past its end (no closing 0xFF)",
            ));
        }
        // Rule 2.4: the state on hooks (presentation, module doc).
    }
}
