// Spec: specs/client/msg-units.md, specs/client/model.md (§2 rule 6, §8, §11, §12 rules 2–3, §14 rule 4, Randomness)
//! Unit messages: add (0x59 players, 0xAC monsters, 0x51 objects),
//! remove (0x0A), re-place (0x15), the queued movement and action
//! messages (0x0C–0x10, 0x4C, 0x4D, 0x67–0x72: a position check, then a
//! mode request) and the local player's vitals (0x18, 0x95, 0x96).
//!
//! What 1.14d also does that is client presentation (gfx, light, UI,
//! automap, music, the mode machines' effects) is Phase 6 and changes no
//! model field.

use std::collections::BTreeMap;

use d2_sim::rng::Seed;

use super::super::bits::BitReader;
use super::super::check::check;
use super::super::dispatch::{HandlerError, Message, UnitMessage};
use super::super::world::{
    room_of_point, ClientUnit, ClientWorld, KindData, ModeRequest, MonsterData, ObjectData,
    PlayerData, UnitKey, INIT_SEED, MISSILE, MONSTER, OBJECT, PLAYER,
};
use super::Bytes;

/// Common creation fields (model §2 rule 6): type, class, GUID, and the
/// seed: {1, 666} at (0, 0); at another point the seed comes from the
/// client room's seed, which the model does not hold (open question 5),
/// so it is `None`.
fn create(key: UnitKey, class: u32, x: u16, y: u16) -> ClientUnit {
    let mut u = ClientUnit::new(key);
    u.class = class;
    let placed = (x, y) != (0, 0);
    // TODO(spec: model.md open question 5): room of the point (§2 rule
    // 7; fatal 0x13C when none), its act, and its seed step.
    u.seed = (!placed).then_some(INIT_SEED);
    u.position = placed.then_some((x, y));
    u
}

/// 0x59 AssignPlayer (§1.1).
pub fn assign_player(w: &mut ClientWorld, msg: &Message<'_>) -> Result<(), HandlerError> {
    let b = Bytes(msg.bytes);
    if msg.bytes.len() != 26 {
        return Err(HandlerError::Invalid("0x59 is 26 bytes"));
    }
    let key = UnitKey::new(PLAYER, b.u32(1)?);
    let (x, y) = (b.u16(0x16)?, b.u16(0x18)?);
    let mut u = create(key, u32::from(b.u8(5)?), x, y);
    // Player init (`0x00460BF0`, rule 3).
    for s in [68, 67, 69] {
        u.stats.insert(s, 100);
    }
    u.mode = 5;
    // Randomness rule 2: unless the new record is already the local
    // player (never: the local player pointer is the old record while the
    // new one is initialised), one step of the unit seed.
    u.seed = u.seed.map(|(lo, hi)| {
        let mut s = Seed::new(lo, hi);
        s.step();
        (s.lo, s.hi)
    });
    let mut name = [0u8; 16];
    name.copy_from_slice(b.slice(6, 16)?);
    u.kind = KindData::Player(PlayerData {
        name,
        cursor_item: None,
    });
    w.add(u);
    Ok(())
}

/// Component bits of a choice count c (§1.2 rule 1): c < 3 → 1 bit; else
/// the bit length of c − 1.
fn component_bits(c: u8) -> u32 {
    if c < 3 {
        1
    } else {
        u8::BITS - (c - 1).leading_zeros()
    }
}

/// 0xAC AssignMonster (§1.2).
pub fn assign_monster(w: &mut ClientWorld, msg: &Message<'_>) -> Result<(), HandlerError> {
    let b = Bytes(msg.bytes);
    let size = usize::from(b.u8(0xC)?);
    if size != msg.bytes.len() || size < 0xD {
        return Err(HandlerError::Invalid("0xAC size byte"));
    }
    let key = UnitKey::new(MONSTER, b.u32(1)?);
    let class = b.u16(5)?;
    let (x, y) = (b.u16(7)?, b.u16(9)?);
    let life = b.u8(0xB)?;
    let mut r = BitReader::new(&msg.bytes[0xD..]);
    // TODO(spec: msg-units.md §1.2 rules 2–3): the hireling GUID is
    // `ClientWorld::hireling_guid`, but the re-initialisation `0x0046EC10`
    // (which fields it resets, whether rules 3–4 then run) and the hireling
    // class test `0x0063EE90` are not stated; every 0xAC takes the creation
    // path.
    let tables = &msg.inputs.tables;
    let class_row = tables.monsters.get(usize::from(class)).copied().flatten();
    let mode = r.read(4);
    let mut data = MonsterData {
        value: -1,
        ..MonsterData::default()
    };
    if r.read(1) == 1 {
        if let Some(row) = class_row {
            for (i, &c) in row.components.iter().enumerate() {
                data.components[i] = r.read(component_bits(c)) as u8;
            }
        }
    }
    if r.read(1) == 1 {
        for bit in [4u8, 8, 2, 0x10, 0x40] {
            if r.read(1) == 1 {
                data.flags |= bit;
            }
        }
        if data.flags & 2 != 0 {
            data.hc_idx = r.read_signed(16) as u16;
        }
        let mut n = 0;
        loop {
            let m = r.read(8) as u8;
            if m == 0 {
                break;
            }
            // 1.14d writes past its 9-byte buffer here (edge case).
            let slot = data.umods.get_mut(n).ok_or(HandlerError::Invalid(
                "0xAC: more umods than the 9-byte buffer",
            ))?;
            *slot = m;
            n += 1;
        }
        data.name_seed = r.read(16) as u16;
        if r.read(1) == 1 {
            data.value = r.read(32) as i32;
        }
    }
    // Rule 2: the class must be a `monstats` row with a `monstats2` row.
    if class_row.is_none() {
        return Ok(());
    }
    let mut u = create(key, u32::from(class), x, y);
    // A monster's seed is init_low(+0x28), {0, 666} without a room.
    if u.position.is_none() {
        u.seed = Some((0, INIT_SEED.1));
    }
    // TODO(spec: msg-units.md open question 2): `0x004AE8D0` (table
    // stats, path, the mode it sets); the model takes the 4-bit mode.
    u.mode = mode;
    // Rule 3 (the hireling exception needs the hireling GUID, see above).
    u.stats.insert(7, 0x8000);
    u.stats.insert(6, i32::from(life) << 8);
    u.stats.insert(328, i32::from(x.wrapping_add(y)));
    // Rule 4.
    if r.read(1) == 1 {
        // TODO(spec: msg-units.md open question 3): what `0x00621CC0`
        // sets.
        data.v31 = Some(r.read(31));
    }
    if r.read(1) == 1 {
        let mut list = BTreeMap::new();
        loop {
            let s = r.read(9);
            let Some(row) = tables.stats.get(s as usize) else {
                break;
            };
            if s >= 0x1FF || row.bits == 0 {
                break;
            }
            let param = if row.param_bits == 0 {
                0
            } else {
                r.read(u32::from(row.param_bits))
            };
            let bits = u32::from(row.bits);
            let value = if bits < 32 && row.signed {
                r.read_signed(bits)
            } else {
                r.read(bits) as i32
            };
            list.insert((s as u16, param as u16), value);
            if r.overflow {
                break;
            }
        }
        if !list.is_empty() {
            data.stat_list = Some(list);
        }
    }
    u.kind = KindData::Monster(Box::new(data));
    w.add(u);
    Ok(())
}

/// 0x51 AssignObject (§1.3).
pub fn assign_object(w: &mut ClientWorld, msg: &Message<'_>) -> Result<(), HandlerError> {
    let b = Bytes(msg.bytes);
    if msg.bytes.len() != 14 {
        return Err(HandlerError::Invalid("0x51 is 14 bytes"));
    }
    let ty = b.u8(1)?;
    if ty == MONSTER {
        return Err(HandlerError::Fatal(0x202));
    }
    if ty > 5 {
        return Err(HandlerError::Invalid("0x51: unit type past 5"));
    }
    let key = UnitKey::new(ty, b.u32(2)?);
    let (x, y) = (b.u16(8)?, b.u16(0xA)?);
    let mut u = create(key, u32::from(b.u16(6)?), x, y);
    if ty == OBJECT {
        u.mode = u32::from(b.u8(0xC)?);
        u.kind = KindData::Object(ObjectData {
            interact: b.u8(0xD)?,
        });
    } else {
        // TODO(spec: msg-units.md §1.2 rule 2): types 0, 3, 4, 5 take
        // their kind's init (player, missile, item, tile); only the object
        // init and its data +4 are stated.
        return Err(HandlerError::Unspecified(
            "msg-units.md §1.3 rule 2: 0x51 for a unit type other than 2",
        ));
    }
    w.add(u);
    Ok(())
}

/// 0x0A RemoveUnit (§2): the local player's hireling (`model.md` §14
/// rule 4) is never removed.
pub fn remove_unit(w: &mut ClientWorld, msg: &Message<'_>) -> Result<(), HandlerError> {
    let b = Bytes(msg.bytes);
    if msg.bytes.len() != 6 {
        return Err(HandlerError::Invalid("0x0A is 6 bytes"));
    }
    let key = UnitKey::new(b.u8(1)?, b.u32(2)?);
    if key.unit_type == MONSTER && key.guid == w.hireling_guid(w.local_player) {
        return Ok(());
    }
    w.remove(key);
    Ok(())
}

/// 0x15 ReassignPlayer (§3): place the unit at (x, y).
pub fn reassign_player(w: &mut ClientWorld, msg: &Message<'_>) -> Result<(), HandlerError> {
    let b = Bytes(msg.bytes);
    if msg.bytes.len() != 11 {
        return Err(HandlerError::Invalid("0x15 is 11 bytes"));
    }
    let key = UnitKey::new(b.u8(1)?, b.u32(2)?);
    let (x, y) = (b.u16(6)?, b.u16(8)?);
    // Rule 2: a unit not in S → nothing.
    if !w.units.contains_key(&key) {
        return Ok(());
    }
    // Rule 4.2: room' := room of (x, y) (`model.md` §12 rule 2); a
    // non-zero point with no room' is fatal 0x168. Without the client DRLG
    // (`active_rooms` none) the point is taken as in a room.
    let new_room = match &w.active_rooms {
        Some(rooms) => {
            let r = room_of_point(rooms, i32::from(x), i32::from(y)).copied();
            if r.is_none() && (x, y) != (0, 0) {
                return Err(HandlerError::Fatal(0x168));
            }
            r
        }
        None => None,
    };
    // TODO(spec: model.md §12 rule 2 a): the cell lookup from the unit's
    // room and its adjacency array runs before the act lookup; the model
    // has no adjacency, so only the act lookup (b) runs.
    // Rule 4.3: a dead unit stays where it is.
    if w.units[&key].is_dead() {
        return Ok(());
    }
    // Rule 2: at (0, 0) the unit ends without a room (fatal 0x538).
    if (x, y) == (0, 0) {
        return Err(HandlerError::Fatal(0x538));
    }
    // Rule 4.4 / `model.md` §11 rule 4: the local player moving to a room
    // whose level's `Act` differs from the old room's switches the act
    // palette; the first placement (no old room) does not.
    if w.local_player == Some(key) {
        if let (Some(old), Some(new)) = (w.local_room().copied(), new_room) {
            let act = |level: u16| {
                msg.inputs
                    .tables
                    .levels
                    .get(usize::from(level))
                    .map(|l| l.act)
                    .ok_or(HandlerError::Invalid("room level past the Levels rows"))
            };
            let new_act = act(new.level)?;
            if act(old.level)? != new_act {
                w.palette_act = Some(new_act);
            }
        }
    }
    // Rule 4.5. TODO(spec: msg-units.md §3 rule 4.5, model.md §12 rule 4):
    // a failed teleport falls back to the nearest free point
    // (`0x0064E7B0`), which needs the client's collision map.
    if let Some(u) = w.units.get_mut(&key) {
        u.position = Some((x, y));
    }
    Ok(())
}

/// One record entry of §4 rule 1.
#[derive(Clone, Copy)]
enum F {
    U8(usize),
    U16(usize),
    U32(usize),
    I16(usize),
    K(i32),
    /// Left unset by 1.14d (stack contents): 0 in the model.
    Unset,
}

/// One row of §4 rule 1: check point, code, record.
struct Row {
    check: Option<(usize, usize)>,
    code: F,
    record: [F; 7],
}

use F::{Unset as X, I16, K, U16, U32, U8};

/// The table of §4 rule 1.
fn row(id: u8) -> Option<Row> {
    let r = |check, code, record| {
        Some(Row {
            check,
            code,
            record,
        })
    };
    match id {
        0x0C => r(None, U8(6), [U8(7), U8(8), K(0), K(0), K(0), K(0), K(0)]),
        0x0D => r(None, U8(6), [U16(7), U16(9), U8(0xB), X, X, X, X]),
        0x0E => r(None, U8(6), [U8(7), U32(8), X, X, X, X, X]),
        0x0F => r(
            Some((0xC, 0xE)),
            U8(6),
            [U16(7), U16(9), U8(0xB), X, X, X, X],
        ),
        0x10 => r(Some((0xC, 0xE)), U8(6), [U8(7), U32(8), X, X, X, X, X]),
        0x4C => r(
            None,
            K(0x16),
            [U16(6), K(-1), U8(9), U32(0xA), U8(8), K(0), K(0)],
        ),
        0x4D => r(
            None,
            K(0x15),
            [U32(6), K(-1), U16(0xB), U16(0xD), U8(0xA), K(0), K(0)],
        ),
        0x67 => r(
            None,
            U8(5),
            [U16(6), U16(8), U8(0xA), U8(0xC), I16(0xD), U8(0xF), U8(0xB)],
        ),
        0x68 => r(
            Some((6, 8)),
            U8(5),
            [
                U8(0xA),
                U32(0xB),
                U8(0xF),
                U8(0x11),
                I16(0x12),
                U8(0x14),
                U8(0x10),
            ],
        ),
        0x69 => r(
            None,
            U8(5),
            [U16(6), U16(8), U8(0xA), K(2), K(0), K(4), U8(0xB)],
        ),
        0x6A => r(None, U8(5), [U8(6), U32(7), U8(0xB), K(2), K(0), K(4), X]),
        0x6B => r(
            Some((0xC, 0xE)),
            U8(5),
            [U16(6), U16(8), U8(0xA), K(2), K(0), K(4), U8(0xB)],
        ),
        0x6C => r(
            Some((0xC, 0xE)),
            U8(5),
            [U8(6), U32(7), U8(0xB), K(2), K(0), K(4), X],
        ),
        0x6D => r(
            Some((5, 7)),
            K(7),
            [U16(5), U16(7), U8(9), K(2), K(0), K(4), K(0)],
        ),
        _ => None,
    }
}

fn read(b: &Bytes<'_>, f: F) -> Result<i32, HandlerError> {
    Ok(match f {
        U8(o) => i32::from(b.u8(o)?),
        U16(o) => i32::from(b.u16(o)?),
        U32(o) => b.u32(o)? as i32,
        I16(o) => i32::from(b.u16(o)? as i16),
        K(v) => v,
        X => 0,
    })
}

/// The unit handlers of §4 rule 1 (with rules 2 and 3): optional position
/// check, then the mode request (model §8).
pub fn queued(w: &mut ClientWorld, msg: &UnitMessage<'_>) -> Result<(), HandlerError> {
    let row = row(msg.id).ok_or(HandlerError::Invalid("no §4 row for this id"))?;
    let b = Bytes(msg.bytes);
    if let Some((xo, yo)) = row.check {
        let (x, y) = (b.u16(xo)?, b.u16(yo)?);
        check(w, msg.inputs, msg.unit, x, y, 0, 0, 0)?;
    }
    // Rule 2 (0x0D, type 0): the party roster's life percent.
    // TODO(spec: msg-units.md open question 4): the roster is the 0x5B
    // owner's; not in the model.
    if msg.id == 0x6D {
        // Rule 3: stat 328 := base(328) + 1.
        if let Some(u) = w.units.get_mut(&msg.unit) {
            let v = u.stat(328).wrapping_add(1);
            u.stats.insert(328, v);
        }
    }
    let code = read(&b, row.code)? as u8;
    let mut record = [0i32; 7];
    for (slot, f) in record.iter_mut().zip(row.record) {
        *slot = read(&b, f)?;
    }
    mode_request(w, msg.unit, code, record);
    Ok(())
}

/// The mode request `0x00480C10` (model §8): stored on the unit until the
/// mode machines are specified (open question 1). A missile's request
/// does nothing in 1.14d; it is not stored.
fn mode_request(w: &mut ClientWorld, key: UnitKey, code: u8, record: [i32; 7]) {
    if key.unit_type == MISSILE {
        return;
    }
    if let Some(u) = w.units.get_mut(&key) {
        u.last_mode_request = Some(ModeRequest { code, record });
    }
}

/// 0x6E–0x72 (§4 rule 4): a bare `ret`.
pub fn no_effect(_: &mut ClientWorld, _: &UnitMessage<'_>) -> Result<(), HandlerError> {
    Ok(())
}

/// The fields of 0x18 / 0x95 / 0x96 (§5 rule 1).
struct Vitals {
    life: Option<u32>,
    mana: Option<u32>,
    stamina: u32,
    ab: Option<(u32, u32)>,
    x: u16,
    y: u16,
    dx: u32,
    dy: u32,
}

/// The signed step of rule 3: dx − 0x100 when dx > 0x80.
fn sdelta(d: u32) -> i32 {
    if d > 0x80 {
        d as i32 - 0x100
    } else {
        d as i32
    }
}

/// 0x18 LifeManaUpdate, 0x95 LifeManaUpdate2, 0x96 WalkVerify (§5).
pub fn vitals(w: &mut ClientWorld, msg: &Message<'_>) -> Result<(), HandlerError> {
    let expected = match msg.id {
        0x18 => 15,
        0x95 => 13,
        _ => 9,
    };
    if msg.bytes.len() != expected {
        return Err(HandlerError::Invalid("vitals message size"));
    }
    let mut r = BitReader::new(msg.bytes);
    r.read(8);
    let v = if msg.id == 0x96 {
        let stamina = r.read(15);
        Vitals {
            life: None,
            mana: None,
            stamina,
            ab: None,
            x: r.read(16) as u16,
            y: r.read(16) as u16,
            dx: r.read(8),
            dy: r.read(8),
        }
    } else {
        let (life, mana, stamina) = (r.read(15), r.read(15), r.read(15));
        let ab = (msg.id == 0x18).then(|| (r.read(7), r.read(7)));
        Vitals {
            life: Some(life),
            mana: Some(mana),
            stamina,
            ab,
            x: r.read(16) as u16,
            y: r.read(16) as u16,
            dx: r.read(8),
            dy: r.read(8),
        }
    };
    let Some(key) = w.local_player.filter(|k| w.units.contains_key(k)) else {
        return Ok(());
    };
    let u = w.units.get_mut(&key).expect("checked above");
    // Rule 2.
    if let Some(life) = v.life {
        u.stats.insert(6, (life << 8) as i32);
    }
    if let Some(mana) = v.mana {
        u.stats.insert(8, (mana << 8) as i32);
    }
    u.stats.insert(10, (v.stamina << 8) as i32);
    if let Some((a, b)) = v.ab {
        u.stats.insert(74, a as i32);
        u.stats.insert(26, b as i32);
    }
    // Rule 3.
    let tx = i32::from(v.x.wrapping_add(sdelta(v.dx) as u16));
    let ty = i32::from(v.y.wrapping_add(sdelta(v.dy) as u16));
    check(w, msg.inputs, key, v.x, v.y, 0, tx, ty)?;
    // Rule 4: leaving the dead mode is the player mode machine's
    // (`0x00480E70`, `0x004647D0`; model.md open question 1): no model
    // field changes.
    Ok(())
}
