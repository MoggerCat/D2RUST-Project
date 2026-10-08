// Spec: specs/world/objects-client.md (§25–§28), specs/client/model.md (§2 rule 1, §5 rules 2–3, §8 rule 7, §18), specs/render/lighting.md (open question 11), specs/render/overlay.md (§5)
//! The client side of objects: the per-object client update `0x004BDFF0`
//! (the generic step `0x004BCBB0`, then the object's `ClientFn` and the
//! mode sound call, call site A), the second `ClientFn` call of a C
//! object (call site B), the 18 client object functions ([`fns`]), the
//! interact sender `0x00480930` ([`interact`]), set C (client-only
//! units) and the latches of §27.
//!
//! Plain Rust, no Bevy. Every timer here reads the update's `now`
//! (`ModelInputs::now`, §25 r6); nothing here reaches the server except
//! the C→S messages appended to `outgoing`. The sound and effect calls
//! leave as [`Output::ObjectSound`] / [`Output::ObjectFx`] in update
//! order (`client/bridge.md` §10).

pub mod fns;
pub mod interact;

#[cfg(test)]
mod tests;

use std::collections::BTreeMap;

use d2_sim::rng::Seed;

use super::dispatch::HandlerError;
use super::output::Output;
use super::world::{ClientUnit, ClientWorld, KindData, ModelInputs, MonsterData, UnitKey, OBJECT};

/// Entries of the client object function table `0x007277F0` (§25 r1).
pub const CLIENT_FNS: u8 = 19;
/// The fatal assert of a `ClientFn` ≥ 19 (`0x004BDEF6`).
pub const FATAL_CLIENT_FN: u32 = 0x546;
/// The client quest record's size (96 bytes, `world/quests-status.md`).
pub const QUEST_RECORD: usize = 0x60;

/// `0x0065C310(rec, q, b)`: bit b of quest q (16 bits per quest, LSB
/// first; `world/quests.md` §1.1).
pub fn quest_bit(rec: &[u8; QUEST_RECORD], q: u8, b: u8) -> bool {
    let n = 16 * usize::from(q) + usize::from(b);
    rec.get(n >> 3).is_some_and(|&v| v & (1 << (n & 7)) != 0)
}

/// Unit flag-ex bit of an expansion game (`model.md` §2 rule 6).
pub const FLAG_EX_EXPANSION: u32 = 0x0200_0000;

/// One `objects.txt` row as the client object code reads it
/// (`world/objects-client.md` Constants: columns read).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ObjClientRow {
    /// `ClientFn` (+0x1B4).
    pub client_fn: u8,
    /// `Start0`–`Start7` (+0x129…), raw bytes (not × 256).
    pub start: [u8; 8],
    /// `FrameCnt0`–`7` (+0xD8), as in the binary row: already × 256
    /// (`data/fixups.md` §13).
    pub frame_cnt: [u32; 8],
    /// `FrameDelta0`–`7` (+0xF8).
    pub frame_delta: [u16; 8],
    /// `CycleAnim0`–`7`.
    pub cycle_anim: [u8; 8],
    /// `SizeX` (+0xD0): the object's unit size (`sim/path-placement.md`
    /// §3).
    pub size_x: u32,
    /// `EnvEffect` (+0x139).
    pub env_effect: u8,
    /// `Selectable0`–`7`.
    pub selectable: [u8; 8],
    /// `Lit0`–`7` (+0x110).
    pub lit: [u8; 8],
    /// `Red`, `Green`, `Blue`.
    pub rgb: (u8, u8, u8),
}

impl ObjClientRow {
    /// The fields of one decoded `objects` row.
    pub fn from_row(o: &d2_data::tables::Objects) -> Self {
        Self {
            client_fn: o.clientfn,
            start: [
                o.start0, o.start1, o.start2, o.start3, o.start4, o.start5, o.start6, o.start7,
            ],
            frame_cnt: [
                o.framecnt0,
                o.framecnt1,
                o.framecnt2,
                o.framecnt3,
                o.framecnt4,
                o.framecnt5,
                o.framecnt6,
                o.framecnt7,
            ],
            frame_delta: [
                o.framedelta0,
                o.framedelta1,
                o.framedelta2,
                o.framedelta3,
                o.framedelta4,
                o.framedelta5,
                o.framedelta6,
                o.framedelta7,
            ],
            cycle_anim: [
                o.cycleanim0,
                o.cycleanim1,
                o.cycleanim2,
                o.cycleanim3,
                o.cycleanim4,
                o.cycleanim5,
                o.cycleanim6,
                o.cycleanim7,
            ],
            size_x: o.sizex,
            env_effect: o.enveffect,
            selectable: [
                o.selectable0,
                o.selectable1,
                o.selectable2,
                o.selectable3,
                o.selectable4,
                o.selectable5,
                o.selectable6,
                o.selectable7,
            ],
            lit: [
                o.lit0, o.lit1, o.lit2, o.lit3, o.lit4, o.lit5, o.lit6, o.lit7,
            ],
            rgb: (o.red, o.green, o.blue),
        }
    }

    /// One row per decoded `objects` row, by class.
    pub fn rows(objects: &[d2_data::tables::Objects]) -> Vec<Self> {
        objects.iter().map(Self::from_row).collect()
    }
}

/// What the client object functions read beside the model.
#[derive(Clone, Debug, Default)]
pub struct ObjClientInputs {
    /// One row per `objects.txt` row, by class. Empty: the headless
    /// configuration; the object update then runs nothing (no row to
    /// read).
    pub rows: Vec<ObjClientRow>,
    /// The client quest flag record `[0x007C0D43]` (`0x004B32D0`), held
    /// by the UI layer (`client/msg-ui.md` §16 r4.1); `None`: no record
    /// (§26.13 r3: fatal 0x1F when read).
    pub quest_flags: Option<[u8; QUEST_RECORD]>,
    /// The code of the local player's item in hand (`0x0063BEF0(P+0x60)`,
    /// `0x00628590`) for the interact sender's classes 404 / 376
    /// (`model.md` §8 rule 7); `None`: no item.
    pub hand_code: Option<[u8; 4]>,
    /// By skill id: the `skills` row byte +7 & `[0x006CE284]` (= 0x80,
    /// `client/stat-lists.md` open question 5), i.e. flag bit 31
    /// `interrupt` (`data/fields.tsv` skills 162), read by the action gate
    /// `0x00480BA0` (§26.13). A skill without an entry: clear.
    pub skill_gate: Vec<bool>,
    /// The byte `0x004538D0(1)` of C→S 0x16 (`model.md` §8 rule 7, item
    /// case), a UI value.
    pub pickup_flag: u8,
}

/// The client latches of §27, part of the client session (§27 r2).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Latches {
    /// `[0x007C025D]`: the orifice preload ran (§26.8).
    pub orifice: bool,
    /// `[0x007C025E]`: the altar preload ran (§26.12).
    pub altar: bool,
    /// `[0x007C025F]`: the zoo is on (S→C 0x50 code 36).
    pub zoo: bool,
    /// `[0x007C0260]`: the zoo has spawned.
    pub zoo_spawned: bool,
    /// `[0x007C0261]`: the GUID of the last chicken (−1 when its create
    /// failed); only the spawn writes it, the reset leaves it.
    pub last_chicken: u32,
    /// `[0x007C0265]`: u16@3 of 0x50 code 36 (never read).
    pub zoo_word: u16,
}

impl Latches {
    /// `0x004A2390` / `0x004A3410` / `0x004A42A0`: the four bytes are
    /// cleared together; `[0x007C0261]` stays (§27 r1).
    pub fn reset(&mut self) {
        self.orifice = false;
        self.altar = false;
        self.zoo = false;
        self.zoo_spawned = false;
    }
}

/// Set C and the object latches (`model.md` §2 rule 1; §27).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ClientObjects {
    /// Set C: the client-only units (unit flag 0x200000), by key.
    pub set_c: BTreeMap<UnitKey, ClientUnit>,
    pub latches: Latches,
    /// The client GUID counter `[0x00711F30]` of `0x00466730`.
    pub next_guid: u32,
}

/// A unit and the set it is in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ObjUnit {
    pub key: UnitKey,
    /// In set C (a client-only unit) rather than set S.
    pub client_only: bool,
}

/// The audio calls of the client object code (`world/objects-client.md`
/// §28 r3), in the order the update makes them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ObjSound {
    /// The mode sound call `0x004CB460(U)` (`audio/triggers.md` §7) with
    /// U's class and mode at the call.
    Mode {
        unit: ObjUnit,
        class: u32,
        mode: u32,
        /// `0x006416D0(U, P)` at the call (`audio/triggers.md` §7 r2;
        /// `audio/triggers-2.md` §20 r5): the path distance to the local
        /// player, [`NO_LOCAL_DISTANCE`] without one.
        local_dist: i32,
    },
    /// A sound request `0x004B9A00(id, U, 0, 0, 0)` (§26.18).
    Request { id: i32, unit: ObjUnit },
    /// The player event sound `0x004CB9C0(P, event)` (`model.md` §8
    /// rule 7, classes 404 / 376 without the item).
    PlayerEvent { player: UnitKey, event: u8 },
}

/// The effect calls of the client object code (§28 r3).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ObjFx {
    /// The graphics refresh `0x00470610(U, 0)` after a mode change.
    GfxRefresh { unit: ObjUnit, mode: u32 },
    /// A graphics load `0x0046F870(class, flag)` (§26.8, §26.12).
    GfxLoad { class: u16, flag: u8 },
    /// Overlay create `0x00470390(U, overlay, kind, 0, 0, 0, 0, 0)`
    /// (`render/overlay.md` §2, §5).
    OverlayCreate {
        unit: ObjUnit,
        overlay: u16,
        kind: u8,
    },
    /// Overlay remove by id `0x0046F0C0(U, 0, overlay)`
    /// (`render/overlay.md` §3 r9).
    OverlayRemove { unit: ObjUnit, overlay: u16 },
    /// The object light `0x004BC580(U, Lit / 2, colour)`
    /// (`render/lighting.md` §8 object row; applied by
    /// `rules::lighting::sources::object_light`).
    Light {
        unit: ObjUnit,
        lit: u8,
        rgb: (u8, u8, u8),
    },
    /// The client skill start `0x004C6EB0(P, R)` (`render/lighting.md`
    /// §8 r3).
    SkillStart { player: UnitKey, record: [i32; 7] },
}

/// The context of one client object call: the model, the inputs, the
/// unit and its class row, and the output list.
pub struct Cx<'a> {
    pub w: &'a mut ClientWorld,
    pub inputs: &'a ModelInputs,
    pub unit: ObjUnit,
    pub row: ObjClientRow,
    pub out: &'a mut Vec<Output>,
}

impl Cx<'_> {
    /// The update's wall clock (§25 r6).
    pub fn now(&self) -> u32 {
        self.inputs.now
    }

    /// U, from its set.
    pub fn u(&mut self) -> Result<&mut ClientUnit, HandlerError> {
        unit_mut(self.w, self.unit).ok_or(HandlerError::Invalid("client object not in its set"))
    }

    /// `step(U)`: one step of U's client seed (§25 r5), the new low word.
    pub fn step(&mut self) -> Result<u32, HandlerError> {
        step_seed(self.u()?)
    }

    /// `range(lo, hi)` (`0x004BC500`, §25 r5).
    pub fn range(&mut self, lo: i32, hi: i32) -> Result<u32, HandlerError> {
        if hi <= lo {
            return Ok(lo as u32);
        }
        let n = hi.wrapping_sub(lo) as u32;
        let r = self.step()?;
        let k = if n.is_power_of_two() {
            r & (n - 1)
        } else {
            r % n
        };
        Ok((lo as u32).wrapping_add(k))
    }

    /// `set_mode(U, m)` (`0x00624690`, §25 r5): a different mode is
    /// written and the animation re-init sets the frame to 0 (frame bonus
    /// 0 for objects); the same mode changes nothing the model holds.
    /// TODO(spec: sim/units.md §4.1): unit flag 1 and the temporary stat
    /// lists are not in the client model.
    pub fn set_mode(&mut self, m: u32) -> Result<(), HandlerError> {
        let u = self.u()?;
        if u.mode != m {
            u.mode = m;
            u.frame = 0;
        }
        Ok(())
    }

    /// `reinit(U)` (`0x00624390`): frame := 0.
    pub fn reinit(&mut self) -> Result<(), HandlerError> {
        self.u()?.frame = 0;
        Ok(())
    }

    /// `refresh(U)` (`0x00470610(U, 0)`): an effect call.
    pub fn refresh(&mut self) -> Result<(), HandlerError> {
        let mode = self.u()?.mode;
        self.fx(ObjFx::GfxRefresh {
            unit: self.unit,
            mode,
        });
        Ok(())
    }

    /// `sound(U)`: the mode sound call `0x004CB460(U)`.
    pub fn sound(&mut self) -> Result<(), HandlerError> {
        let u = self.u()?;
        let (class, mode) = (u.class, u.mode);
        let local_dist = local_distance(self.w, self.inputs, self.unit);
        self.out.push(Output::ObjectSound(ObjSound::Mode {
            unit: self.unit,
            class,
            mode,
            local_dist,
        }));
        Ok(())
    }

    /// `End(m)` = `FrameCnt[m]` − 256 (§25 r5).
    pub fn end(&self, m: u32) -> Result<i32, HandlerError> {
        let cnt = frame_cnt(&self.row, m)?;
        Ok((cnt as i32).wrapping_sub(256))
    }

    pub fn fx(&mut self, fx: ObjFx) {
        self.out.push(Output::ObjectFx(fx));
    }
}

/// `FrameCnt[m]`; a mode past 7 reads beyond the eight columns.
fn frame_cnt(row: &ObjClientRow, m: u32) -> Result<u32, HandlerError> {
    row.frame_cnt
        .get(m as usize)
        .copied()
        .ok_or(HandlerError::Invalid(
            "object mode past the eight objects.txt modes",
        ))
}

/// One step of a unit's client seed; the new low word.
pub fn step_seed(u: &mut ClientUnit) -> Result<u32, HandlerError> {
    let (lo, hi) = u.seed.ok_or(HandlerError::Invalid(
        "client seed not in the model (no client DRLG)",
    ))?;
    let mut s = Seed::new(lo, hi);
    let r = s.step();
    u.seed = Some((s.lo, s.hi));
    Ok(r)
}

/// The unit of `r`, from set S or set C.
pub fn unit_mut(w: &mut ClientWorld, r: ObjUnit) -> Option<&mut ClientUnit> {
    if r.client_only {
        w.objclient.set_c.get_mut(&r.key)
    } else {
        w.units.get_mut(&r.key)
    }
}

pub fn unit_ref(w: &ClientWorld, r: ObjUnit) -> Option<&ClientUnit> {
    if r.client_only {
        w.objclient.set_c.get(&r.key)
    } else {
        w.units.get(&r.key)
    }
}

/// U's class row; `None` in the headless configuration (no rows).
fn row_of(
    w: &ClientWorld,
    inputs: &ModelInputs,
    r: ObjUnit,
) -> Result<Option<ObjClientRow>, HandlerError> {
    let rows = &inputs.objclient.rows;
    if rows.is_empty() {
        return Ok(None);
    }
    let Some(u) = unit_ref(w, r) else {
        return Ok(None);
    };
    rows.get(u.class as usize)
        .copied()
        .map(Some)
        .ok_or(HandlerError::Invalid(
            "object class without an objects.txt row",
        ))
}

/// The generic object step `0x004BCBB0` (`render/lighting.md` §8;
/// `world/objects-client.md` §26.6: frame += speed, a non-cycling mode
/// clamps at its last frame).
///
/// PROVISIONAL (objects-client.md §26.16; REC-45): the speed is the
/// class's `FrameDelta[mode]` (the speed source `0x00470610` is not
/// traced); a cycling mode wraps (frame − `FrameCnt`, modulo); a mode
/// with `FrameCnt` 0 does not advance; and the end of a non-cycling mode
/// 1 sets mode 2 (`set_mode`, then the graphics refresh), the transition
/// §26.2, §26.7 and §26.16 name.
/// TODO(spec: render/lighting.md §8): the `Lit2` light of the generic
/// step is not made here.
pub fn generic_step(cx: &mut Cx<'_>) -> Result<(), HandlerError> {
    let m = cx.u()?.mode;
    let cnt = frame_cnt(&cx.row, m)? as i32;
    if cnt <= 0 {
        return Ok(());
    }
    let i = m as usize;
    let speed = i32::from(cx.row.frame_delta[i]);
    let cycle = cx.row.cycle_anim[i] != 0;
    let u = cx.u()?;
    u.frame = u.frame.wrapping_add(speed);
    if u.frame < cnt {
        return Ok(());
    }
    if cycle {
        u.frame = u.frame.rem_euclid(cnt);
        return Ok(());
    }
    u.frame = cnt.wrapping_sub(256);
    if m == 1 {
        cx.set_mode(2)?;
        cx.refresh()?;
    }
    Ok(())
}

/// The dispatch `0x004BDEE0` (§25 r1): `ClientFn` 0 → 1 without a call;
/// ≥ 19 → fatal 0x546; else the table entry's result.
pub fn dispatch(cx: &mut Cx<'_>) -> Result<bool, HandlerError> {
    fns::call(cx)
}

/// The object update `0x004BDFF0` (call site A, §25 r2; `model.md` §5
/// rule 2): the generic step, then `ClientFn` ≤ 3 → the mode sound call
/// only; ≥ 4 → the dispatch, then the mode sound call when it returned
/// non-zero. Nothing without object rows.
pub fn object_update(
    w: &mut ClientWorld,
    inputs: &ModelInputs,
    unit: ObjUnit,
    out: &mut Vec<Output>,
) -> Result<(), HandlerError> {
    let Some(row) = row_of(w, inputs, unit)? else {
        return Ok(());
    };
    let mut cx = Cx {
        w,
        inputs,
        unit,
        row,
        out,
    };
    generic_step(&mut cx)?;
    if row.client_fn <= 3 {
        return cx.sound();
    }
    if dispatch(&mut cx)? {
        cx.sound()?;
    }
    Ok(())
}

/// Call site B (§25 r3; `model.md` §5 rule 3): after a C unit's update,
/// a type-2 unit still in set C runs the dispatch once more, result
/// ignored.
/// TODO(spec: client/model.md §5 rule 3): type 1 runs `0x0046D780`
/// (not specified).
pub fn site_b(
    w: &mut ClientWorld,
    inputs: &ModelInputs,
    key: UnitKey,
    out: &mut Vec<Output>,
) -> Result<(), HandlerError> {
    if key.unit_type != OBJECT || !w.objclient.set_c.contains_key(&key) {
        return Ok(());
    }
    let unit = ObjUnit {
        key,
        client_only: true,
    };
    let Some(row) = row_of(w, inputs, unit)? else {
        return Ok(());
    };
    let mut cx = Cx {
        w,
        inputs,
        unit,
        row,
        out,
    };
    dispatch(&mut cx)?;
    Ok(())
}

/// The order of a C set walk (`0x00463CC0`, `model.md` §5 rule 3):
/// buckets `GUID & 0x7F` ascending, each chain in descending GUID.
pub fn c_order(w: &ClientWorld, unit_type: u8) -> Vec<UnitKey> {
    let mut keys: Vec<UnitKey> = w
        .objclient
        .set_c
        .keys()
        .copied()
        .filter(|k| k.unit_type == unit_type)
        .collect();
    keys.sort_by_key(|k| (k.guid & 0x7F, std::cmp::Reverse(k.guid)));
    keys
}

/// The client-only creator `0x00466730(class, x, y, …)` (flags 0x600000,
/// `model.md` §2 rule 1): a new unit in set C with a GUID from the client
/// counter `[0x00711F30]` and the creation fields of `model.md` §2 rule
/// 6. Returns its key; `None` when the create fails (the point is in no
/// room of the client DRLG).
///
/// PROVISIONAL (objects-client.md §26.17; REC-objclient-1): the counter
/// starts at 0 and the new GUID is the counter's value before a += 1
/// (the counter's start and step are not specified).
pub fn create_client_unit(
    w: &mut ClientWorld,
    unit_type: u8,
    class: u32,
    x: u16,
    y: u16,
) -> Option<UnitKey> {
    let guid = w.objclient.next_guid;
    w.objclient.next_guid = guid.wrapping_add(1);
    let key = UnitKey::new(unit_type, guid);
    let mut u = ClientUnit::new(key);
    u.class = class;
    if w.expansion != 0 {
        u.flag_ex |= FLAG_EX_EXPANSION;
    }
    let placed = (x, y) != (0, 0);
    if placed && w.drlg.is_some() {
        let r = w.room_at(x, y)?;
        let seed = w.drlg.as_mut().and_then(|d| d.unit_seed(r.room))?;
        u.seed = Some((seed.lo, seed.hi));
    } else if placed {
        // No client DRLG: the room's seed is not in the model
        // (`msg/units.rs` `create`).
        u.seed = None;
    }
    u.position = placed.then_some((x, y));
    if unit_type == super::world::MONSTER {
        u.kind = KindData::Monster(Box::new(MonsterData {
            hc_idx: class as u16,
            ..MonsterData::default()
        }));
    }
    // Insert (`model.md` §2 rule 3) fails on a key already in the set.
    if w.objclient.set_c.contains_key(&key) {
        return None;
    }
    w.objclient.set_c.insert(key, u);
    Some(key)
}

/// The client-only removal `0x00465F00(GUID, type)` (`model.md` §5 rule
/// 5): unlink from set C and free; a key not in set C: nothing. The free
/// is recorded for its `UnitFreed` output (`client/bridge.md` §10 r3.1
/// (a): every unit free, set C too).
pub fn remove_client_unit(w: &mut ClientWorld, key: UnitKey) -> Option<ClientUnit> {
    let u = w.objclient.set_c.remove(&key)?;
    w.freed.push((key, true));
    Some(u)
}

/// `0x006416D0(a, b)` (`missiles/missiles.md` §R9.5) over client units:
/// per axis |Δ| − (size(a) / 2 + size(b) / 2), floored at 0, then
/// (2·max + min) / 2. Positions are the client cells.
pub fn distance(a: &ClientUnit, size_a: i32, b: &ClientUnit, size_b: i32) -> i32 {
    let (ax, ay) = a.cell();
    let (bx, by) = b.cell();
    d2_sim::world::objects::chests::reach_distance(
        (i32::from(ax), i32::from(ay)),
        size_a,
        (i32::from(bx), i32::from(by)),
        size_b,
    )
}

/// The distance of an [`ObjSound::Mode`] without a local player (or with
/// U gone): no rule reads it as near.
/// PROVISIONAL (audio/triggers.md §7 r2; REC-51): 1.14d has no update
/// pass without a local player in practice; read as far.
pub const NO_LOCAL_DISTANCE: i32 = i32::MAX;

/// `0x006416D0(U, P)` for an object U and the local player P (the mode
/// sound's distance, `audio/triggers.md` §7 r2).
pub fn local_distance(w: &ClientWorld, inputs: &ModelInputs, unit: ObjUnit) -> i32 {
    let rows = &inputs.objclient.rows;
    let u = if unit.client_only {
        w.objclient.set_c.get(&unit.key)
    } else {
        w.units.get(&unit.key)
    };
    match (u, w.local()) {
        (Some(u), Some(p)) => distance(u, unit_size(u, rows), p, unit_size(p, rows)),
        _ => NO_LOCAL_DISTANCE,
    }
}

/// The unit size of `0x00620510` for a client unit (`sim/path-placement.md`
/// §3): player 2, object `SizeX`, item 1, tile 0.
/// TODO(spec: client/model.md): monster and missile sizes need
/// `monstats2` / `missiles` columns the client tables do not hold; 0.
pub fn unit_size(u: &ClientUnit, rows: &[ObjClientRow]) -> i32 {
    match u.key.unit_type {
        super::world::PLAYER => 2,
        OBJECT => rows.get(u.class as usize).map_or(0, |r| r.size_x as i32),
        super::world::ITEM => 1,
        _ => 0,
    }
}
