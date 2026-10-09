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
    // r6.1: colorshift ≠ 0 → the colour call (its model part: the local
    // player's light colour, `render/shading.md` §6 r1.1).
    colour_call(w, inputs, key, row)
}

/// The colour call of state on / off (r6.1, r6.3) when the state's
/// `colorshift` ≠ 0; its row is read only when the call can change a
/// light (the local player with a light).
fn colour_call(
    w: &mut ClientWorld,
    inputs: &ModelInputs,
    key: UnitKey,
    row: Option<super::super::world::StateRow>,
) -> Result<(), HandlerError> {
    if !super::lighting::colour_call_lights(w, key) {
        return Ok(());
    }
    let row = row.ok_or(HandlerError::Invalid(
        "client/stat-lists.md §3 r6: no states row for the state's colorshift",
    ))?;
    if row.colorshift == 0 {
        return Ok(());
    }
    super::lighting::state_colour_light(w, &inputs.tables.states, key)
}

/// State off `0x00639DB0(unit, state, 0)`: the bit only.
pub fn state_bit_off(w: &mut ClientWorld, key: UnitKey, state: u8) {
    if let Some(u) = w.units.get_mut(&key) {
        u.states.remove(&state);
    }
}

/// State off with its list (0xA9: `0x004D9F40` then `0x004D9C30`): the
/// bit cleared, the state's list detached and freed, then the colour call
/// for a state with `colorshift` ≠ 0 (r6.3).
pub fn state_off(
    w: &mut ClientWorld,
    inputs: &ModelInputs,
    key: UnitKey,
    state: u8,
) -> Result<(), HandlerError> {
    let Some(u) = w.units.get_mut(&key) else {
        return Ok(());
    };
    u.states.remove(&state);
    u.state_lists.remove(&state);
    // r6.3: colorshift ≠ 0 → the colour call after the bit is cleared.
    let row = inputs.tables.states.get(usize::from(state)).copied();
    colour_call(w, inputs, key, row)
}

/// The state stat `0x004D9D70(unit, list, state, stat, value, param)`
/// (`client/stat-lists.md` §3 r2): the unit's list of the state (made
/// and attached when it has none), then the set `0x00627150`
/// (`sim/stat-lists.md` §5 r1: a value 0 removes the entry).
/// Stat 172 also runs `0x00463C00(value)` (§3 r2): a monster with old ≠
/// new (bytes; old = the existing list's stat 172 at that param, 4 for a
/// new list) and a room changes the room's allied count (+0x28): new = 2
/// → += 1, else old = 2 → −= 1 (fatal 0x27C when already < 1); the path
/// reset `0x00649CA0` has no model field.
pub fn state_stat(
    w: &mut ClientWorld,
    key: UnitKey,
    state: u8,
    stat: u16,
    param: u16,
    value: i32,
) -> Result<(), HandlerError> {
    if stat == 172 && key.unit_type == MONSTER {
        let old = w
            .units
            .get(&key)
            .and_then(|u| u.state_lists.get(&state))
            .map_or(4, |l| l.get(&(172, param)).copied().unwrap_or(0)) as u8;
        let new = value as u8;
        if let Some(room) = w.room_units.room_of(key).filter(|_| old != new) {
            let count = w.room_allied.entry(room).or_insert(0);
            if new == 2 {
                *count += 1;
            } else if old == 2 {
                if *count < 1 {
                    return Err(HandlerError::Fatal(0x27C));
                }
                *count -= 1;
            }
        }
    }
    if let Some(u) = w.units.get_mut(&key) {
        let list = u.state_lists.entry(state).or_default();
        if value == 0 {
            list.remove(&(stat, param));
        } else {
            list.insert((stat, param), value);
        }
    }
    Ok(())
}

/// Why a state stat stream stopped.
enum StatsEnd {
    /// 0x1FF: the state's hooks run.
    Done,
    /// A stat outside the table or with `Send Bits` 0.
    Abort,
    /// A fatal assert of a state stat (§3 r2).
    Fatal(HandlerError),
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
        if let Err(e) = state_stat(w, key, state, id as u16, param, value) {
            return StatsEnd::Fatal(e);
        }
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
        state_off(w, msg.inputs, key, state)
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
    if let StatsEnd::Fatal(e) = read_stats(w, msg.inputs, key, state, &mut r) {
        return Err(e);
    }
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
            match read_stats(w, msg.inputs, key, state, &mut r) {
                // Rule 2.3: the whole message ends, no hooks.
                StatsEnd::Abort => return Ok(()),
                StatsEnd::Fatal(e) => return Err(e),
                StatsEnd::Done => {}
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
