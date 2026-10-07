// Spec: specs/audio/triggers.md
// Spec: specs/audio/object-sounds.tsv
// Spec: specs/audio/npc-speech.tsv
//! Sound triggers: which code path requests which sound (`triggers.md`).
//! One plain-Rust function per cause class (`client/audio.md` §A2), each
//! with explicit inputs, the sound fields of `triggers.md` §1 r6 in
//! [`UnitSound`] / [`Globals`], and every call to the sound layer through
//! [`SoundCalls`] (plus the read-only row/request queries of
//! [`TriggerSound`]). No Bevy, no float, no I/O; all draws on the client
//! RNG through [`SoundCalls::roll`].
//!
//! - §2 server sound events, §3 player event sounds: [`events`]
//! - §4 mode sounds: [`modes`]
//! - §5 footsteps, §6 monster idle voices: [`movement`]
//! - §7 object mode sounds: [`objects`] (table [`tables::ObjectSounds`])
//! - §8 skills/missiles/states, §9 items: [`skills`]
//! - §10 NPC speech: [`npc`] (table [`tables::NpcSpeech`])
//! - §11 UI sounds, §12 other fixed requests: [`ui`]

pub mod events;
pub mod modes;
pub mod movement;
pub mod npc;
pub mod objects;
pub mod skills;
pub mod tables;
pub mod ui;

#[cfg(test)]
mod tests;

use d2_data::tables::Monsounds;

use super::calls::{Handle, SoundCalls};
use crate::bridge::world::UnitKey;

pub use crate::bridge::world::{MONSTER, OBJECT, PLAYER};

/// Read-only queries the rules need beyond the [`SoundCalls`] surface:
/// `sounds.txt` row data and the request/unit lists the sound layer owns
/// (`sound-table.md` §4 r1, §5). Implemented by the sound table; tests use
/// a recording fake.
pub trait TriggerSound: SoundCalls {
    /// Sound system on (`[0x007C545C]`, `sound-table.md` §5 r1). §6 r1.1
    /// reads it (its group test is false while the system is off).
    fn sound_on(&self) -> bool;
    /// Group base of a sound id (`sounds` +0x60, `sound-table.md` §4 r1).
    fn group_base(&self, id: i32) -> i32;
    /// The sound row's `Loop` column (§7 r4, `0x004CAA10`).
    fn looping(&self, id: i32) -> bool;
    /// U+0x78: the requests attached to U that have not ended, in list
    /// order, as (handle, requested id) (§1 r6).
    fn unit_requests(&self, unit: UnitKey) -> Vec<(Handle, i32)>;
    /// Number of units in a request's unit list (+0x28; §4.3 r2.2).
    fn unit_count(&self, h: Handle) -> usize;
    /// Variant pick `0x00482680(id)` with its draws (`sound-table.md` §4
    /// r3), used by the NPC greeting (§10 r1).
    fn variant(&mut self, id: i32) -> i32;
}

/// A fatal path of the original (an assert / crash in 1.14d). Returned,
/// never swallowed (M07).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TriggerError {
    #[error("player class {0} >= 7 (class record table, §3 r1)")]
    PlayerClass(i32),
    #[error("swing index {0} >= 14 (§1 r9)")]
    SwingIndex(u8),
    #[error("object class {0} > 572 (§7 r1)")]
    ObjectClass(i32),
    #[error("footstep step F / n = 0 (F = {frames}, n = {count}; §5 r4 division by zero)")]
    FootstepStep { frames: u32, count: u32 },
}

/// The client unit as the rules read it (§1 r6, r10; `sim/units.md` §4).
/// Fields a rule does not use may stay at their defaults.
#[derive(Clone, Copy, Debug)]
pub struct Unit<'a> {
    pub key: UnitKey,
    /// Player class, monstats row, object class or item row (+0x04).
    pub class: i32,
    /// Monster `BaseId` (`0x00463860`; §6 r1).
    pub base_class: i32,
    /// Current mode (+0x10).
    pub mode: u8,
    /// +0x44 frame (fixed point, as stored).
    pub frame: u32,
    /// +0x48 frame count.
    pub frame_count: u32,
    /// +0x4C animation speed.
    pub speed: i32,
    /// U is the local player P (`[0x007A6A70]`).
    pub is_local: bool,
    /// Monster's `monsounds` record (`0x004CA410`), if any.
    pub monsounds: Option<&'a Monsounds>,
    /// `monstats2.critter` (`0x004638A0(class, 13)`; §4.2 r2).
    pub critter: bool,
    /// Has state 146 (§4.2 r1).
    pub state_146: bool,
    /// Has state 1 `freeze` (§4.2 r5).
    pub frozen: bool,
    /// Dying or dead (`0x00464820` ≠ 0; §6 r2).
    pub dying: bool,
    /// Weapon hit class (`0x00623C20`; §4.3 r1, §8 r1).
    pub weapon_hit_class: u8,
    /// U has a room and its level is a town (`0x0061AB00`; §6 r1.4).
    pub in_town: bool,
    /// Within 700 of P (`0x004B9E10`, see [`within_700`]; §6 r1.5).
    pub near_local: bool,
    /// Distance to P (`0x006416D0`; §7 r2).
    pub local_dist: i32,
}

impl<'a> Unit<'a> {
    pub fn new(key: UnitKey, class: i32) -> Self {
        Unit {
            key,
            class,
            base_class: class,
            mode: 0,
            frame: 0,
            frame_count: 0,
            speed: 0,
            is_local: false,
            monsounds: None,
            critter: false,
            state_146: false,
            frozen: false,
            dying: false,
            weapon_hit_class: 0,
            in_town: false,
            near_local: false,
            local_dist: 0,
        }
    }

    pub fn unit_type(&self) -> u8 {
        self.key.unit_type
    }
}

/// `0x004B9E10`: x² + (2y)² < 490,000 for the client position deltas of U
/// relative to P (`sound-table.md` §8.1 r1 positions; §6 r1.5).
pub fn within_700(dx: i64, dy: i64) -> bool {
    dx * dx + (2 * dy) * (2 * dy) < 490_000
}

/// Per-unit sound fields (§1 r6), kept by the client for each unit.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct UnitSound {
    /// +0x70 object mode seen.
    pub obj_seen: bool,
    /// +0x74 object previous mode.
    pub obj_prev_mode: u8,
    /// +0x7C last voice (C).
    pub last_voice: u32,
    /// +0x80 last idle voice (C).
    pub last_idle: u32,
    /// +0x84 last footstep (C).
    pub last_footstep: u32,
    /// +0xB0 last hit class taken (open question 3: writer unknown).
    pub hit_class: u8,
}

/// Global sound timers (§1 r6). All start 0.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Globals {
    /// `[0x007C88B8]` last idle voice of any monster (C).
    pub last_idle_any: u32,
    /// `[0x007C88BC]` last voice of any unit (C).
    pub last_voice_any: u32,
    /// `[0x007C88C0]` idle gap (C).
    pub idle_gap: u32,
    /// `[0x007C88C4]` time (C) of the last player speech line (§3 r6).
    pub speech_time: u32,
    /// `[0x007C88C8]` id of the last player speech line (§3 r6).
    pub speech_id: i32,
}

/// What a rule call works with: the sound layer, the globals and the
/// client update counter C (§1 r5).
pub struct Ctx<'a> {
    pub s: &'a mut dyn TriggerSound,
    pub g: &'a mut Globals,
    /// C = `[0x007A0498]`.
    pub c: u32,
}

impl<'a> Ctx<'a> {
    pub fn new(s: &'a mut dyn TriggerSound, g: &'a mut Globals, c: u32) -> Self {
        Ctx { s, g, c }
    }

    /// request(id, U, d, f, o) (§1 r1).
    fn req(&mut self, id: i32, unit: Option<UnitKey>, delay: u32) -> Handle {
        self.s.request(id, unit, delay, 0, 0)
    }

    /// U+0x7C := C, `[0x007C88BC]` := C.
    fn voiced(&mut self, us: &mut UnitSound) {
        us.last_voice = self.c;
        self.g.last_voice_any = self.c;
    }
}

/// Swing table `0x00727ED8` (§4.3 r1): (id, frames) per index `h & 0xF`,
/// id 0 = none. Indices 14, 15 are outside the table (fatal, §1 r9).
pub const SWING: [(i32, u32); 14] = [
    (0, 0),
    (251, 6),
    (262, 6),
    (268, 6),
    (274, 8),
    (280, 8),
    (286, 6),
    (292, 8),
    (268, 6),
    (298, 6),
    (0, 0),
    (0, 0),
    (0, 0),
    (0, 0),
];

/// Swing table entry for hit class `h` (`0x004CA4C0`).
pub fn swing_entry(h: u8) -> Result<(i32, u32), TriggerError> {
    SWING
        .get((h & 0xF) as usize)
        .copied()
        .ok_or(TriggerError::SwingIndex(h & 0xF))
}

/// `swing(U, h)` (`0x004CA500`, §1 r9): (F × 256 + 128) / max(speed, 1),
/// unsigned, truncating.
pub fn swing(speed: i32, h: u8) -> Result<u32, TriggerError> {
    let (_, frames) = swing_entry(h)?;
    Ok(delay_ticks(frames, speed))
}

/// (F × 256 + 128) / max(speed, 1) (§1 r9, §4.3 r2.3).
pub fn delay_ticks(frames: u32, speed: i32) -> u32 {
    frames.wrapping_mul(256).wrapping_add(128) / speed.max(1) as u32
}

/// Player class record `[0x0072A008 + 4·class]` (§3 r1), offsets +0x00 …
/// +0x28 in field order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClassRecord {
    pub hit: i32,
    pub death: i32,
    pub impossible: i32,
    pub needmana: i32,
    pub needkey: i32,
    pub cantcarry: i32,
    pub cantuseyet: i32,
    pub notintown: i32,
    pub chat_base: i32,
    pub quest_base: i32,
    pub footstep_base: i32,
}

const fn rec(v: [i32; 11]) -> ClassRecord {
    ClassRecord {
        hit: v[0],
        death: v[1],
        impossible: v[2],
        needmana: v[3],
        needkey: v[4],
        cantcarry: v[5],
        cantuseyet: v[6],
        notintown: v[7],
        chat_base: v[8],
        quest_base: v[9],
        footstep_base: v[10],
    }
}

/// §3 r1 table (checked against the spec's table by a test, M05).
pub const CLASS_RECORDS: [ClassRecord; 7] = [
    rec([
        2878, 2883, 2953, 2957, 2955, 2934, 2936, 2959, 2937, 2961, 2768,
    ]),
    rec([
        2910, 2915, 3265, 3269, 3267, 3246, 3248, 3271, 3249, 3273, 2720,
    ]),
    rec([
        2894, 2899, 3109, 3113, 3111, 3090, 3092, 3115, 3093, 3117, 2720,
    ]),
    rec([
        2886, 2891, 3187, 3191, 3189, 3168, 3170, 3193, 3171, 3195, 2768,
    ]),
    rec([
        2902, 2907, 3031, 3035, 3033, 3012, 3014, 3037, 3015, 3039, 2816,
    ]),
    rec([
        2926, 2931, 3421, 3425, 3423, 3402, 3404, 3427, 3405, 3429, 2720,
    ]),
    rec([
        2918, 2923, 3343, 3347, 3345, 3324, 3326, 3349, 3327, 3351, 2720,
    ]),
];

/// Class record of a player class; class ≥ 7 (or negative) is fatal.
pub fn class_record(class: i32) -> Result<&'static ClassRecord, TriggerError> {
    usize::try_from(class)
        .ok()
        .and_then(|c| CLASS_RECORDS.get(c))
        .ok_or(TriggerError::PlayerClass(class))
}

/// A `monsounds` sound column as the signed id the rules compare.
pub(crate) fn sid(v: u32) -> i32 {
    v as i32
}

/// Detach with force every request of U whose group base is one of
/// `ids`' group bases (`0x004CB2C0` §4.6 r1; `0x004CB190` §10 r1, r2).
fn detach_groups(s: &mut dyn TriggerSound, unit: UnitKey, ids: &[i32]) {
    let bases: Vec<i32> = ids.iter().map(|&id| s.group_base(id)).collect();
    for (h, id) in s.unit_requests(unit) {
        if bases.contains(&s.group_base(id)) {
            s.detach(h, unit, true);
        }
    }
}

/// The skill voices of a monster record (`Skill1..4`).
fn skill_voices(r: &Monsounds) -> [i32; 4] {
    [sid(r.skill1), sid(r.skill2), sid(r.skill3), sid(r.skill4)]
}

/// `0x004CB190(N)`: detach N's skill voices with force (§10 r1, r2).
/// TODO(spec: audio/triggers.md §10 r1): "skill voices" read as the
/// groups of `monsounds.Skill1..4` of N's record.
pub fn detach_skill_voices(s: &mut dyn TriggerSound, unit: UnitKey, rec: Option<&Monsounds>) {
    if let Some(r) = rec {
        detach_groups(s, unit, &skill_voices(r));
    }
}

/// `[0x007C88C0]` := uniform(30, 90) (§6 r1, r2).
pub const IDLE_GAP_RANGE: (i32, i32) = (30, 90);

fn uniform_gap(cx: &mut Ctx) -> u32 {
    super::calls::uniform(&mut *cx.s, IDLE_GAP_RANGE.0, IDLE_GAP_RANGE.1) as u32
}
