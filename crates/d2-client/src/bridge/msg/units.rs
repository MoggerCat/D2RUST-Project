// Spec: specs/client/msg-units.md, specs/client/model.md (§2 rule 6, §8, §11, §12 rules 2–3, §14 rule 4, §15, Randomness), specs/sim/unit-order.md (§5 rule 6)
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
use super::super::drlg::DrlgRoomId;
use super::super::modes::{mode_request, neutral_walk, player_mode, remove_unit_light};
use super::super::objects::interact::{mode_request_code_2, CODE_INTERACT};
use super::super::objects::FLAG_EX_EXPANSION;
use super::super::output::{Output, ShrineFxKind};
use super::super::player_anim;
use super::super::skills::SkillList;
use super::super::world::{
    ClientUnit, ClientWorld, KindData, MonsterData, MonsterSetup, ObjectData, PlayerData, UnitKey,
    INIT_SEED, MONSTER, OBJECT, PLAYER,
};
use super::Bytes;

/// A unit being created and the room of its point.
struct Created {
    unit: ClientUnit,
    room: Option<DrlgRoomId>,
}

impl Created {
    /// Adds the unit (§2 rule 4) and links it at the head of its creation
    /// room's list when it has one (`sim/unit-order.md` §5 rule 6: the
    /// dynamic path set-up of players and monsters, the object init).
    fn add(self, w: &mut ClientWorld) {
        let key = self.unit.key;
        w.add(self.unit);
        if self.room.is_some() {
            w.room_units.place(key, self.room);
        }
    }
}

/// Common creation fields (model §2 rule 6): type, class, GUID, and the
/// seed: {1, 666} at (0, 0); at another point the room of the point
/// (§2 rule 7, fatal 0x13C when none) has its seed stepped once and the
/// unit seed is `init_low(lo')` (§12 rule 5; 0x59, 0xAC and 0x51 alike,
/// `msg-units.md` Randomness). Without a client DRLG the room's seed is
/// not in the model, so the seed is `None`.
fn create(
    w: &mut ClientWorld,
    key: UnitKey,
    class: u32,
    x: u16,
    y: u16,
) -> Result<Created, HandlerError> {
    let mut u = ClientUnit::new(key);
    u.class = class;
    if w.expansion != 0 {
        u.flag_ex |= FLAG_EX_EXPANSION;
    }
    let placed = (x, y) != (0, 0);
    u.seed = (!placed).then_some(INIT_SEED);
    let mut room = None;
    if placed && w.drlg.is_some() {
        let r = w.room_at(x, y).ok_or(HandlerError::Fatal(0x13C))?;
        let seed = w
            .drlg
            .as_mut()
            .and_then(|d| d.unit_seed(r.room))
            .expect("a listed room is active");
        u.seed = Some((seed.lo, seed.hi));
        room = Some(r.room);
    }
    u.position = placed.then_some((x, y));
    Ok(Created { unit: u, room })
}

/// 0x59 AssignPlayer (§1.1).
pub fn assign_player(w: &mut ClientWorld, msg: &Message<'_>) -> Result<(), HandlerError> {
    let b = Bytes(msg.bytes);
    if msg.bytes.len() != 26 {
        return Err(HandlerError::Invalid("0x59 is 26 bytes"));
    }
    let key = UnitKey::new(PLAYER, b.u32(1)?);
    let (x, y) = (b.u16(0x16)?, b.u16(0x18)?);
    let mut c = create(w, key, u32::from(b.u8(5)?), x, y)?;
    let u = &mut c.unit;
    // Player init (`0x00460BF0`, rule 3).
    for s in [68, 67, 69] {
        u.stats.insert(s, 100);
    }
    // A skill list (`0x006438B0`, +0xA8; `msg-skills.md` §1 rule 1),
    // then its native skills (`0x00647EE0`, `msg-skills.md` §2 rule 8).
    let mut list = SkillList::default();
    let owner = crate::bridge::skills::Owner {
        unit_type: PLAYER,
        class: u.class,
    };
    let class_skills = msg.inputs.tables.class_skills.get(u.class as usize);
    crate::bridge::skills::init_player(&mut list, &msg.inputs.tables.skills, owner, class_skills)
        .map_err(HandlerError::from)?;
    u.skills = Some(list);
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
        ..PlayerData::default()
    });
    c.add(w);
    // Player init (`0x00460BF0`): the player light (`render/lighting.md`
    // §8 player row).
    super::lighting::player_light(w, key);
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

/// State 98 `sourceunit` (`msg-units.md` §1.2 r4).
const SOURCE_UNIT_STATE: u8 = 98;

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
    // Rule 2: the local player's hireling (GUID = `0x00478F20(local
    // player, 7)`, monster in S) is re-initialised, not created (r2.1:
    // graphics rebuilt, mode := the 4-bit mode; nothing else of the
    // message is written); rules 3 and 4 then run on it.
    let hireling = w.hireling_guid(w.local_player);
    let reinit = key.guid == hireling && w.units.contains_key(&key);
    let mut created = None;
    if reinit {
        w.units.get_mut(&key).expect("checked above").mode = mode;
    } else {
        // Rule 2: the class must be a `monstats` row with a `monstats2`
        // row.
        if class_row.is_none() {
            return Ok(());
        }
        let mut c = create(w, key, u32::from(class), x, y)?;
        let u = &mut c.unit;
        // A monster's seed is init_low(+0x28), {0, 666} without a room.
        if u.position.is_none() {
            u.seed = Some((0, INIT_SEED.1));
        }
        // Rule 6: the monster set-up `0x004AE8D0`.
        let setup = class_row.and_then(|c| c.setup);
        if let Some(s) = &setup {
            setup_stats(u, s, w.difficulty, w.expansion != 0);
        }
        // Rule 6.4: the mode argument is the 4-bit mode.
        u.mode = mode;
        // Rule 6.5 (the frame draw on the unit seed with range +0x48)
        // runs once the unit is added ([`super::super::monster_anim`]).
        // Rule 6.9 (the direction draw) needs `0x0046C140`, which the
        // client tables do not hold: not run
        // (`docs/handoff/impl-c-client.md` §4).
        if let Some(s) = &setup {
            setup_flags(u, s);
        }
        // Rule 6.7: `+0xA8` := a skill list (`0x006438B0(0)`).
        u.skills = Some(SkillList::default());
        u.kind = KindData::Monster(Box::new(data));
        if let Some(s) = &setup {
            let bonus = tables.monster_skill_bonus[usize::from(w.difficulty.min(2))];
            setup_skills(u, s, &tables.skills, bonus)?;
        }
        created = Some(c);
    }
    // Rule 3 on U (the new unit, or the re-initialised hireling).
    let apply3 = |u: &mut ClientUnit| {
        // `0x0063EE90` tests U's own class.
        let own_hireling = matches!(u.class, 271 | 338 | 359 | 560 | 561);
        if !(own_hireling && key.guid == hireling) {
            u.stats.insert(7, 0x8000);
            u.stats.insert(6, i32::from(life) << 8);
        }
        if own_hireling && u.mode == 1 {
            u.flag_ex &= !0x40000;
        }
        u.stats.insert(328, i32::from(x.wrapping_add(y)));
    };
    // Rule 4: the source-unit link and the 0x40 stat list.
    let link = (r.read(1) == 1).then(|| r.read(31));
    let mut list = BTreeMap::new();
    if r.read(1) == 1 {
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
    }
    let apply4 = |u: &mut ClientUnit| {
        if let Some(v) = link {
            // `0x00621CC0(unit, 0, v)` → `0x00621C30` (`skills/bodies.md`
            // §6.20): +0x94 := 0 (owner type player), +0x98 := v, state
            // 98 `sourceunit` with stats 353 := 0, 354 := v (the unit has
            // a stat holder), flag-ex |= 0x400.
            u.states.insert(SOURCE_UNIT_STATE);
            let l = u.state_lists.entry(SOURCE_UNIT_STATE).or_default();
            l.remove(&(353, 0));
            l.insert((354, 0), v as i32);
            u.flag_ex |= 0x400;
        }
        if let KindData::Monster(d) = &mut u.kind {
            if let Some(v) = link {
                d.v31 = Some(v);
            }
            if !list.is_empty() {
                // The flag-0x40 list: an existing one is reused.
                d.stat_list
                    .get_or_insert_with(BTreeMap::new)
                    .extend(list.clone());
            }
        }
    };
    match created {
        Some(mut c) => {
            apply3(&mut c.unit);
            apply4(&mut c.unit);
            c.add(w);
            // Rules 6.4–6.5: the mode set's animation part (frame 0, the
            // mode's count, the rate), then the first frame drawn from the
            // unit seed.
            super::super::monster_anim::mode_restart(w, msg.inputs, key, mode);
            super::super::monster_anim::first_frame(w, key);
            // Rule 6.9: the initial path direction (after the frame draw:
            // the other order gives 40 / 29 where 1.14d draws 57 / 46,
            // `gen-render-firebolt` / `-frozen`).
            if let Some(row) = &class_row {
                super::super::monster_anim::first_direction(w, key, row.npc, row.modes);
            }
            // The monster init's light (`0x004AE210`, `render/lighting.md`
            // §8 monster row), in the monster's room.
            if let Some(row) = &class_row {
                super::lighting::monster_light(w, key, row);
            }
            // Rule 6.8's assigns owe the passive-state parts of a passive
            // skill (`msg-skills.md` §2 r4), applied on the added unit.
            let fx = w
                .units
                .get_mut(&key)
                .and_then(|u| u.skills.as_mut())
                .map(|l| std::mem::take(&mut l.fx))
                .unwrap_or_default();
            super::super::passive::apply(w, msg.inputs, key, fx)?;
        }
        None => {
            let u = w.units.get_mut(&key).expect("checked above");
            apply3(u);
            apply4(u);
        }
    }
    Ok(())
}

/// Rule 6.1 (`0x004AE8D0` first part): the base stats of the set-up, `d`
/// = the difficulty; the classic scaling `0x0063EEF0`
/// (`monsters/init.md` §13) in a classic game with d > 0 and `Align` ≠ 1
/// gives level += 25·d (maxhp, armor and experience are not in the
/// model; rule 3 overwrites 6 and 7).
pub(crate) fn setup_stats(u: &mut ClientUnit, s: &MonsterSetup, difficulty: u8, expansion: bool) {
    let d = usize::from(difficulty.min(2));
    let mut level = i32::from(s.level[d]);
    if !expansion && d > 0 && s.align != 1 {
        level += 25 * d as i32;
    }
    u.stats.insert(12, level);
    u.stats.insert(68, 100);
    u.stats.insert(67, 75);
    u.stats.insert(69, 100);
    for (stat, res) in [36u16, 37, 39, 41, 43, 45].into_iter().zip(s.res) {
        u.stats.insert(stat, i32::from(res[d] as i16));
    }
    u.stats.insert(7, 0x6400);
    u.stats.insert(6, 0x6400);
}

/// Rule 6.6: the unit flags from `monstats2`: 0x2 := `isSel`, 0x20 :=
/// not `shadow` (no model field: the shadow is render state), 0x8 set
/// (render), 0x4 := `isAtt`.
pub(crate) fn setup_flags(u: &mut ClientUnit, s: &MonsterSetup) {
    u.flag_2 = Some(s.is_sel);
    u.flag_4 = s.is_att;
}

/// Rule 6.8: `Skill`i ≥ 0 with level byte > 0 → assign at level byte +
/// the act level bonus (`0x00647280`), the entry's mode := `Sk`i`mode`
/// (`0x00644340`).
fn setup_skills(
    u: &mut ClientUnit,
    s: &MonsterSetup,
    rows: &[super::super::world::SkillRow],
    bonus: i32,
) -> Result<(), HandlerError> {
    let owner = super::super::skills::Owner {
        unit_type: MONSTER,
        class: u.class,
    };
    let list = u.skills.get_or_insert_with(SkillList::default);
    for (skill, lvl, mode) in s.skills {
        if skill < 0 || lvl == 0 {
            continue;
        }
        let skill = skill as u16;
        super::super::skills::assign(list, rows, owner, skill, i32::from(lvl) + bonus, false)?;
        if let Some(i) = list.native(skill) {
            list.entries[i].mode = u32::from(mode);
        }
    }
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
    let mut c = create(w, key, u32::from(b.u16(6)?), x, y)?;
    let u = &mut c.unit;
    if ty != OBJECT {
        // Rule 5: 1.14d never sends types 0, 3, 4, 5 (the one builder
        // passes type 2); refused like type 1.
        return Err(HandlerError::Invalid(
            "0x51: unit type 0, 3, 4 or 5 (never sent by 1.14d, msg-units.md §1.3 rule 5)",
        ));
    }
    let class = u.class;
    let interact = b.u8(0xD)?;
    u.mode = u32::from(b.u8(0xC)?);
    // Rule 3: a shrine (`0x00621B00`: objects `SubClass` bit 0) gets the
    // shrines record of index interact (`0x006414B0`; out of range →
    // fatal 0x15F / 0x160), then `0x004BD650` runs the code's on-mode
    // function. Without the class's `objects.txt` row the shrine test
    // cannot run: the shrine part is skipped (as `shrine_facts`).
    let is_shrine = msg
        .inputs
        .tables
        .objects
        .get(class as usize)
        .is_some_and(|r| r.subclass & 1 != 0);
    let shrine_code = if is_shrine {
        let code = *msg
            .inputs
            .tables
            .shrines
            .get(usize::from(interact))
            .ok_or(HandlerError::Fatal(0x15F))?;
        if code >= SHRINES {
            return Err(HandlerError::Fatal(0x37B));
        }
        Some(code)
    } else {
        None
    };
    u.kind = KindData::Object(ObjectData {
        interact,
        shrine: shrine_code,
        ..ObjectData::default()
    });
    // The object init's animation set-up in the mode byte
    // (`world/objects-client.md` §25 r8; measured REC-440: 1.14d's
    // `0x004BC720` runs `0x00624390` in the 0x51 mode, `facts/objects/objanim-a1-town.tsv`), on the
    // unit's client seed. Nothing without rows.
    if let Some(row) = msg.inputs.objclient.rows.get(class as usize) {
        crate::bridge::objects::anim_setup(u, row, u.mode)?;
        // Rule 2: the init stamps the footprint (`0x00620A70`) when
        // `HasCollision[mode]` is set (a mode past 7 reads as 0, as
        // `d2_sim::path::record::ObjectShape::collides_in`).
        let footprint = row.shape.collides_in(u.mode);
        if let KindData::Object(d) = &mut u.kind {
            d.footprint = footprint;
        }
    }
    let mode = u.mode;
    c.add(w);
    // Rule 2's object init `0x004BC720` gives the object its light
    // (`0x004BC580`, `render/lighting.md` §8 object row: `Lit<mode>` / 2,
    // kind 2). Without the class's row the light cannot be read: nothing.
    if let Some(row) = msg.inputs.tables.objects.get(class as usize).copied() {
        let lit = *row.lit.get(mode as usize).ok_or(HandlerError::Invalid(
            "0x51: object mode past the eight objects.txt modes",
        ))?;
        super::lighting::object_light(w, key, lit, row.rgb);
    }
    if let Some(code) = shrine_code {
        let e = shrine(code);
        if e.on_mode {
            msg.out.push(Output::ShrineFx {
                kind: ShrineFxKind::OnMode,
                code,
                object: key,
                player: None,
                overlays: e.overlays,
            });
        }
    }
    Ok(())
}

/// 0x09 AssignLevelWarp (§7 r1): type u8@1, GUID u32@2, class u8@6,
/// x u16@7, y u16@9. Created with the common fields (the room seed
/// step at a point other than (0, 0)) and added; the kind inits are not
/// modelled (the model of §7 r1 is a unit with `class` and `position`).
/// `0x00470B70` for a type-1 unit (no 1.14d sender passes type 1) writes
/// no model field §7 r1 names (as after 0xAC, §1.2 r3).
pub fn assign_level_warp(w: &mut ClientWorld, msg: &Message<'_>) -> Result<(), HandlerError> {
    let b = Bytes(msg.bytes);
    if msg.bytes.len() != 11 {
        return Err(HandlerError::Invalid("0x09 is 11 bytes"));
    }
    let ty = b.u8(1)?;
    if ty > 5 {
        return Err(HandlerError::Invalid("0x09: unit type past 5"));
    }
    let key = UnitKey::new(ty, b.u32(2)?);
    let (x, y) = (b.u16(7)?, b.u16(9)?);
    let c = create(w, key, u32::from(b.u8(6)?), x, y)?;
    c.add(w);
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
    // Rule 4.2: room' := room of (x, y) (`model.md` §12 rule 2: the cell
    // lookup from the local player's room, then the act lookup); a
    // non-zero point with no room' is fatal 0x168. Without the client DRLG
    // (`active_rooms` none) the point is taken as in a room.
    let new_room = match &w.active_rooms {
        Some(_) if (x, y) == (0, 0) => None,
        Some(_) => Some(w.room_at(x, y).ok_or(HandlerError::Fatal(0x168))?),
        None => None,
    };
    // Rule 4.3: a dead unit stays where it is.
    if w.units[&key].is_dead() {
        return Ok(());
    }
    // Rule 2: at (0, 0) the unit ends without a room (fatal 0x538).
    if (x, y) == (0, 0) {
        return Err(HandlerError::Fatal(0x538));
    }
    // Rule 4.4 / `model.md` §11 rule 4: the local player moving to a room
    // whose level's `Pal` (+0x02, not `Act`) differs from the old room's
    // switches the palette to `Pal`; the first placement (no old room)
    // does not.
    if w.local_player == Some(key) {
        if let (Some(old), Some(new)) = (w.local_room().copied(), new_room) {
            let pal = |level: u16| {
                msg.inputs
                    .tables
                    .levels
                    .get(usize::from(level))
                    .map(|l| l.pal)
                    .ok_or(HandlerError::Invalid("room level past the Levels rows"))
            };
            let new_pal = pal(new.level)?;
            if pal(old.level)? != new_pal {
                w.palette_act = Some(new_pal);
            }
        }
    }
    // Rule 4.5. TODO(spec: msg-units.md §3 rule 4.5, model.md §12 rule 4):
    // a failed teleport falls back to the nearest free point
    // (`0x0064E7B0`), which needs the client's collision map.
    if let Some(u) = w.units.get_mut(&key) {
        u.position = Some((x, y));
        u.placements = u.placements.wrapping_add(1);
    }
    // Rule 4.6: the local player's placement ends in `0x00472C20(flag)`.
    if w.local_player == Some(key) {
        w.local_places.push(b.u8(10)?);
    }
    // The teleport's room recache (`sim/unit-order.md` §5 rule 6): leave
    // the old room's list, head of room''s.
    if w.active_rooms.is_some() {
        w.room_units.place(key, new_room.map(|r| r.room));
    }
    // Rule 6, the last call `0x00463B80`: a player with a room in mode 1 or
    // 5 gets the mode request code 7 with no record (`0x00461250`), which
    // sets the neutral mode of the room it now stands in (5 in town, else
    // 1): a warp out of town leaves the town neutral mode at once.
    if key.unit_type == PLAYER && w.room_units.room_of(key).is_some() {
        let mode = w.units[&key].mode;
        if mode == player_mode::NEUTRAL || mode == player_mode::TOWN_NEUTRAL {
            remove_unit_light(w, key);
            let (neutral, _) = neutral_walk(w, key);
            w.units.get_mut(&key).expect("checked above").flag_2 = Some(true);
            player_anim::mode_set(w, msg.inputs, key, neutral);
        }
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
    // Rule 2 (0x0D, type 0): the party roster's life percent (§8 r6).
    if msg.id == 0x0D && msg.unit.unit_type == PLAYER {
        let life = b.u8(0xC)?;
        if let Some(i) = w.roster_find(msg.unit.guid) {
            w.roster[i].life = u32::from(life);
        }
    }
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
    mode_request(w, msg.inputs, msg.unit, code, record, msg.out)?;
    // Player code 0x02 (model §8 rule 4): the interact sender
    // `0x00480930(r0 & 0xFFFF, r1)` (§8 rule 7).
    if msg.unit.unit_type == PLAYER && code == CODE_INTERACT && w.units.contains_key(&msg.unit) {
        for o in mode_request_code_2(w, msg.inputs, record)? {
            msg.out.push(o);
        }
    }
    // Objects (rule 5, `model.md` §15): the shrine part of codes 3 and
    // 0x15, after the stored request.
    if msg.unit.unit_type == OBJECT {
        match code {
            3 => shrine_on_mode(w, msg)?,
            0x15 => shrine_on_use(w, msg, record[0] as u32)?,
            _ => {}
        }
    }
    Ok(())
}

/// One entry of the shrine table `0x006DA8C0` (`model.md` §15 rule 2).
#[derive(Clone, Copy)]
struct Shrine {
    on_mode: bool,
    on_use: bool,
    overlays: [i32; 2],
    sound: u32,
}

/// The shrine count `[0x0072779C]`.
const SHRINES: u8 = 23;

/// The shrine table (`model.md` §15 rule 2), by shrine code.
fn shrine(code: u8) -> Shrine {
    const N: [i32; 2] = [-1, -1];
    let (on_mode, on_use, overlays, sound) = match code {
        0 => (false, false, N, 0),
        1 | 2 => (false, false, N, 0xA71),
        3 => (false, false, N, 0xA70),
        4 | 5 => (false, false, N, 0xA6A),
        6 => (true, false, [0x3B, 0x39], 0xA68),
        7 => (true, false, [0x3C, 0x39], 0xA69),
        8 => (true, false, [0x3E, 0x39], 0xA73),
        9 => (true, false, [0x3F, 0x3A], 0xA72),
        10 => (true, false, [0x3D, 0x3A], 0xA74),
        11 => (true, false, [0x40, 0x3A], 0xA75),
        12 => (true, false, [0x41, 0x39], 0xA77),
        13 => (true, false, [0x42, 0x3A], 0xA70),
        14 => (true, false, [0x43, 0x3A], 0xA70),
        15 => (true, false, [0x44, 0x39], 0xA6B),
        16 => (false, true, N, 0xA76),
        17 | 20 => (false, false, N, 0xA6F),
        18 => (false, false, N, 0xA6D),
        19 => (false, true, N, 0xA78),
        21 => (false, true, N, 0xA6C),
        _ => (false, true, N, 0xA6E),
    };
    Shrine {
        on_mode,
        on_use,
        overlays,
        sound,
    }
}

/// The object's `objects.txt` row and its shrine code (`model.md` §15
/// rule 1): `(is shrine, shrine data Code, ShrineFunction)`. `None` when
/// the client tables hold no row for the class (as 0xAC without
/// `monstats2` rows, the shrine part is skipped); the app supplies the
/// user's `objects.txt` and `shrines.txt` rows (`ClientTables`).
fn shrine_facts(w: &ClientWorld, msg: &UnitMessage<'_>) -> Option<(bool, Option<u8>, u8)> {
    let u = w.units.get(&msg.unit)?;
    let row = msg.inputs.tables.objects.get(u.class as usize)?;
    let data = match &u.kind {
        KindData::Object(d) => d.shrine,
        _ => None,
    };
    Some((row.subclass & 1 != 0, data, row.shrine_function))
}

/// Code 3 for an object (`model.md` §15 rule 3): after the mode change
/// (the stored request), a shrine's on-mode function `0x004BD650`.
fn shrine_on_mode(w: &mut ClientWorld, msg: &UnitMessage<'_>) -> Result<(), HandlerError> {
    let Some((is_shrine, data, _)) = shrine_facts(w, msg) else {
        return Ok(());
    };
    if !is_shrine {
        return Ok(());
    }
    let code = data.ok_or(HandlerError::Fatal(0x37A))?;
    if code >= SHRINES {
        return Err(HandlerError::Fatal(0x37B));
    }
    let e = shrine(code);
    if e.on_mode {
        msg.out.push(Output::ShrineFx {
            kind: ShrineFxKind::OnMode,
            code,
            object: msg.unit,
            player: None,
            overlays: e.overlays,
        });
    }
    Ok(())
}

/// Code 0x15 for an object (`0x004BD5C0`, `model.md` §15 rule 4): r0 is
/// the operator's GUID.
fn shrine_on_use(
    w: &mut ClientWorld,
    msg: &UnitMessage<'_>,
    operator: u32,
) -> Result<(), HandlerError> {
    // Step 1.
    let Some((is_shrine, data, function)) = shrine_facts(w, msg) else {
        return Ok(());
    };
    let (code, data) = if is_shrine {
        let code = data.ok_or(HandlerError::Invalid(
            "model.md §15 rule 4.1: a shrine without shrine data (a null read in 1.14d)",
        ))?;
        (code, data)
    } else {
        (function, None)
    };
    // Step 2.
    let player = UnitKey::new(PLAYER, operator);
    if !w.units.contains_key(&player) {
        return Ok(());
    }
    // Step 3.
    if code > 0 && code < SHRINES && shrine(code).on_use {
        msg.out.push(Output::ShrineFx {
            kind: ShrineFxKind::OnUse,
            code,
            object: msg.unit,
            player: Some(player),
            overlays: shrine(code).overlays,
        });
    }
    // Step 4 (`0x004BD550`).
    let code = data.ok_or(HandlerError::Fatal(0x34D))?;
    if code >= SHRINES {
        return Err(HandlerError::Fatal(0x34E));
    }
    let sound = shrine(code).sound;
    if sound != 0 {
        msg.out.push(Output::ShrineSound { sound, player });
    }
    Ok(())
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
