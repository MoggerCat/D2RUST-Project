// Spec: specs/sim/intents-events.md §9, §2.3, §2.4
//! The small C→S handlers `intents-events.md` §9 owns (rules 2–14):
//! 0x12 EndInferno, 0x14 OverheadChat, 0x3D HighlightDoor (message part),
//! 0x3F PlayAudio, 0x41 Resurrect, 0x44 StaffInOrifice (entry), 0x46
//! MercInteract, 0x47 MoveMerc, 0x48 TurnOffBusyState, 0x4B
//! RequestEntityUpdate, 0x4D PlayNpcMessage, 0x51 BindHotkey, 0x53
//! StaminaOn, 0x54 StaminaOff and 0x60 SwapWeapons (message part).
//!
//! The dispatcher (`crate::dispatch`) has run the gate and the exact
//! size check (§2.3, §2.4 rule 1) when [`handle`] runs. Each handler's
//! own rules are [`run`], on the game state the world host lends as a
//! [`PlayerWorld`] ([`WorldHost::player`]; the action wiring's provider
//! is [`action::ActionPlayer`]). A host without one leaves the ids to
//! the stub, as does a handler whose behaviour sits behind a call with
//! no provider ([`Outcome::code`] `None`: 0x44's `0x00549520`, 0x60's
//! `0x005616A0`).
//!
//! The unit and point tests a handler makes with a range of its own
//! (0x3D range 10, 0x46 / 0x47 range 50: `0x00548F80`, `0x00548EF0`,
//! §2.4 rules 3–4) read the same staged facts as the dispatcher's
//! parsers ([`crate::seams::Intents::unit_target`]) and are made here
//! before the world call ([`Run::target`]).
//!
//! The hot-key slots 0x51 fills are client data (client +0x3DC):
//! [`SimGame::hotkeys`].

pub mod action;

#[cfg(test)]
mod tests;

use d2_sim::skills::SkillEntry;
use d2_sim::tick::EventDispatch;
use d2_sim::units::{UnitId, UnitType};

use super::super::SimGame;
use super::world::{WorldError, WorldFault, WorldHost};
use crate::seams::{ClientId, Intents, MessageSink, Pos, ResultCode, UnitTarget};

/// How a client id of this module is handled.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    /// Its rules are [`run`]'s.
    Handled,
    /// Owned by another spec; no `d2-sim` code to route it to yet: stub.
    Pending,
}

/// Every C→S id whose owner `intents-events.md` §9 rule 1 names and that
/// no other handler module routes: (id, `client-messages.tsv` name,
/// owner, status). The order is the id order.
pub const PLAYER_IDS: &[(u8, &str, &str, Status)] = &[
    (
        0x12,
        "EndInferno",
        "sim/intents-events.md §9 r2",
        Status::Handled,
    ),
    (
        0x14,
        "OverheadChat",
        "sim/intents-events.md §9 r3",
        Status::Handled,
    ),
    // Relay: §9 open question 14; the checks are the dispatcher's.
    (
        0x15,
        "Chat",
        "sim/intents-events.md §2.4 r6 (relay: open question 14)",
        Status::Pending,
    ),
    (
        0x3D,
        "HighlightDoor",
        "sim/intents-events.md §9 r4 (0x005845D0: open question 15)",
        Status::Handled,
    ),
    (
        0x3F,
        "PlayAudio",
        "sim/intents-events.md §9 r5",
        Status::Handled,
    ),
    (
        0x41,
        "Resurrect",
        "sim/intents-events.md §9 r6",
        Status::Handled,
    ),
    (
        0x44,
        "StaffInOrifice",
        "world/quests-act2.md §8.6 (entry: sim/intents-events.md §9 r7)",
        Status::Handled,
    ),
    (
        0x46,
        "MercInteract",
        "sim/intents-events.md §9 r8",
        Status::Handled,
    ),
    (
        0x47,
        "MoveMerc",
        "sim/intents-events.md §9 r8",
        Status::Handled,
    ),
    (
        0x48,
        "TurnOffBusyState",
        "sim/intents-events.md §9 r9",
        Status::Handled,
    ),
    (
        0x4B,
        "RequestEntityUpdate",
        "sim/intents-events.md §9 r10",
        Status::Handled,
    ),
    (
        0x4D,
        "PlayNpcMessage",
        "sim/intents-events.md §9 r11",
        Status::Handled,
    ),
    (
        0x51,
        "BindHotkey",
        "sim/intents-events.md §9 r12",
        Status::Handled,
    ),
    (
        0x53,
        "StaminaOn",
        "sim/intents-events.md §9 r13",
        Status::Handled,
    ),
    (
        0x54,
        "StaminaOff",
        "sim/intents-events.md §9 r13",
        Status::Handled,
    ),
    // `monsters/ai-bodies.md` §9.9 sets the AI params; the handler's
    // entry (unit lookup, refusals) is not written.
    (
        0x59,
        "MakeEntityMove",
        "monsters/ai-bodies.md §9.9 (entry not written)",
        Status::Pending,
    ),
    (
        0x60,
        "SwapWeapons",
        "sim/intents-events.md §9 r14 (0x005616A0: open question 16)",
        Status::Handled,
    ),
];

/// Whether `id` is routed to [`run`].
pub fn handled(id: u8) -> bool {
    PLAYER_IDS
        .iter()
        .any(|&(i, _, _, s)| i == id && s == Status::Handled)
}

/// State 12 `inferno` (§9 rule 2).
pub const STATE_INFERNO: u16 = 12;
/// State 0x36 (54, uninterruptable), on then off in 0x41 (§9 rule 6).
pub const STATE_UNINTERRUPTABLE: u16 = 0x36;
/// Player modes read and set here (`units.md` §1).
pub const MODE_NEUTRAL: u32 = 1;
pub const MODE_WALK: u32 = 2;
pub const MODE_RUN: u32 = 3;
pub const MODE_DEAD: u32 = 0x11;
/// Stats 6, 8, 10: life, mana, stamina (§9 rule 6; stamina also rule 13).
pub const STAT_LIFE: u16 = 6;
pub const STAT_MANA: u16 = 8;
pub const STAT_STAMINA: u16 = 10;
/// Unit flags (+0xC4) and flags-ex (+0xC8) bits §9 writes or reads.
pub const FLAG_RESURRECTED: u32 = 0x2;
pub const FLAG_OVERHEAD: u32 = 0x100;
pub const FLAG_NO_AUDIO: u32 = 0x400;
pub const FLAG_EX_RESEND: u32 = 0x1_0000;
/// Timer event 6 (free the overhead, `tick.md` §5).
pub const EVENT_OVERHEAD: u32 = 6;
/// The overhead duration: 8 frames per character (at most 254), plus 0x7D.
pub const OVERHEAD_PER_CHAR: i32 = 8;
pub const OVERHEAD_MAX_CHARS: i32 = 254;
pub const OVERHEAD_BASE: i32 = 0x7D;
/// 0x3F's speech sounds.
pub const AUDIO_SOUNDS: std::ops::RangeInclusive<u16> = 25..=32;
/// The mercenary's sound event toward the player (§9 rule 8).
pub const SOUND_MERC_ACK: u32 = 15;
/// The mercenary AI commands of 0x46 / 0x47.
pub const COMMAND_INTERACT: i32 = 0x0C;
pub const COMMAND_MOVE: i32 = 0x0D;
/// Ranges of the unit / point tests (§9 rules 4, 8).
pub const DOOR_RANGE: u32 = 10;
pub const MERC_RANGE: u32 = 50;
/// The drop reason of a hardcore resurrect (`0x0052CAF0(…, 3)`).
pub const DROP_HARDCORE: u32 = 3;
/// Hot-key slots (client +0x3DC, 16 × 8 bytes).
pub const HOTKEY_SLOTS: usize = 16;

/// One hot-key slot (client +0x3DC + 8·slot): skill i16 (−1: unbound),
/// left-hand byte at +2, item u32 at +4 (§9 rule 12; read by the join's
/// S→C 0x7B, §8.2 rule 3.6).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HotKey {
    pub skill: i16,
    pub left: bool,
    pub item: u32,
}

impl HotKey {
    /// An unbound slot.
    ///
    /// A brand-new character's client record holds skill −1 in all 16
    /// slots, so the join sends no 0x7B (`intents-events.md` §8.2 r3.6;
    /// recorded, REC-02: `facts/join/a1-new-ama.tsv`, `a1-new-sor.tsv`).
    pub const UNBOUND: HotKey = HotKey {
        skill: -1,
        left: false,
        item: 0,
    };
}

/// The game state the §9 handlers read and write, as the world host
/// lends it. Each method names its 1.14d function; its provider is
/// [`action::ActionPlayer`] (the action wiring), which answers what no
/// written spec holds through `d2_sim::wiring::action::Pending`.
pub trait PlayerWorld {
    fn frame(&self) -> i32;
    /// Game +0x70 (expansion).
    fn expansion(&self) -> bool;
    /// Game +0x6D (difficulty).
    fn difficulty(&self) -> u8;
    /// Unit +0x10.
    fn mode(&self, u: UnitId) -> u32;
    /// `0x00624690(unit, mode)` (`units.md` §4.1, without the combat-list
    /// drop of `0x00553570`).
    fn set_mode(&mut self, u: UnitId, mode: u32);
    /// Unit flags +0xC4.
    fn flags(&self, u: UnitId) -> u32;
    fn or_flags(&mut self, u: UnitId, bits: u32);
    /// Unit flags-ex +0xC8.
    fn or_flags_ex(&mut self, u: UnitId, bits: u32);
    /// `0x00625480(unit, stat, 0)`.
    fn stat(&self, u: UnitId, stat: u16) -> i32;
    /// `0x00639DB0(unit, state, on)`: the toggle (`stat-lists.md` §9.2)
    /// with the update-queue insert.
    fn set_state(&mut self, u: UnitId, state: u16, on: bool);
    /// `0x0064C040` (`unit-order.md` §6.2).
    fn queue_update(&mut self, u: UnitId);
    /// `0x005417D0(type, expire, game, unit, 0, 0)` (`tick.md` §5.2).
    fn schedule_event(&mut self, u: UnitId, event: u32, expire: i32);
    /// `0x00553380(unit, sound, to)`.
    fn sound(&mut self, u: UnitId, sound: u32, to: Option<UnitId>);
    /// `0x00552F60(game, type, GUID)`.
    fn find_unit(&self, ty: UnitType, guid: u32) -> Option<UnitId>;
    /// The room test of 0x4B: the player's room is in the unit's room's
    /// room list (`0x0065A590` → `0x00619790`).
    fn room_in_list_of(&self, player: UnitId, unit: UnitId) -> bool;
    /// `0x005541B0`.
    fn is_dead(&self, u: UnitId) -> bool;
    /// `0x00620250` ≠ null.
    fn has_used_skill(&self, u: UnitId) -> bool;
    /// The skills count (data tables +0xBA0).
    fn skill_count(&self) -> u32;
    /// `0x006439B0(unit, skill, item)` ≠ 0.
    fn has_skill(&self, u: UnitId, skill: i32, item: u32) -> bool;
    /// The monstats row count (data tables +0xA80).
    fn monstats_count(&self) -> u32;

    // ---- 0x14, 0x3D
    /// `0x00413490(text, −1)` ≠ 0.
    fn overhead_text_test(&self, text: &[u8]) -> bool;
    /// Free the unit's overhead record and make a new one ending at
    /// `end` with byte +8 = `byte8`, stored at unit +0xA4 (`0x006611A0`,
    /// `0x00661110`, `0x00661230`).
    fn replace_overhead(&mut self, u: UnitId, text: &[u8], byte8: u8, end: i32);
    /// `0x005845D0(game, player, GUID)`.
    fn highlight_door(&mut self, player: UnitId, guid: u32);

    // ---- 0x41
    /// Client flag 4 (`0x00538670`).
    fn hardcore(&self, player: UnitId) -> bool;
    /// `0x0052CAF0(game, client, reason)`.
    fn drop_client(&mut self, player: UnitId, reason: u32);
    /// The unit's skill list (unit +0xA8), in list order.
    fn skill_entries(&self, u: UnitId) -> Vec<SkillEntry>;
    /// skills.txt `passivestate` of `skill` (signed; ≤ 0: none).
    fn passive_state(&self, skill: i32) -> i32;
    /// `0x00646D60(unit, entry)`.
    fn passive_state_apply(&mut self, u: UnitId, e: &SkillEntry);
    /// `0x00625D10` / `0x00625D60` / `0x00625DB0` for stats 6 / 8 / 10.
    fn stat_max(&self, u: UnitId, stat: u16) -> i32;
    /// `0x00627260(unit, stat, value, 0)`, then its message (`0x00548520`).
    fn set_stat_send(&mut self, u: UnitId, stat: u16, value: i32);
    /// The level id of the unit's room (none: 0).
    fn current_level(&self, u: UnitId) -> u32;
    /// `0x0053AEC0(game, player, level, arg)` (`path-placement.md` §11).
    fn warp(&mut self, player: UnitId, level: u32, arg: u32);
    /// `0x005809D0(game, player, no target, mode, 0, 0, skip gate 1)`.
    fn start_mode_skip_gate(&mut self, player: UnitId, mode: u32);
    /// `0x005701B0` on the left skill with EDX = 1, then the right with
    /// EDX = 0.
    fn reselect_hand_skills(&mut self, player: UnitId);

    // ---- 0x44
    /// `0x00535060`.
    fn busy(&self, player: UnitId) -> bool;
    /// `0x005678A0(…, 1)`.
    fn trading(&self, player: UnitId) -> bool;
    /// `0x00549520(game, player, object, item, action)`; `None`: no
    /// provider.
    fn staff_in_orifice(
        &mut self,
        player: UnitId,
        object: u32,
        item: u32,
        action: u16,
    ) -> Option<u32>;

    // ---- 0x46, 0x47
    /// `0x00574EC0(game, player, 7, 0)`.
    fn hireling(&self, player: UnitId) -> Option<UnitId>;
    /// `0x0058EDE0` (free the AI commands), then `0x0058EF40` (add
    /// `command`).
    fn replace_ai_commands(&mut self, monster: UnitId, command: [i32; 5]);

    // ---- 0x48, 0x4D, 0x60
    /// Player data +0x4C ≠ 0.
    fn busy_flag(&self, player: UnitId) -> bool;
    /// `0x005350B0`: player data +0x4C := 0.
    fn clear_busy_flag(&mut self, player: UnitId);
    /// `0x005724C0`.
    fn clear_npc_intro(&mut self, player: UnitId, difficulty: u8, class: u16);
    /// `0x005616A0(game, player, &fail)`: (result, fail); `None`: no
    /// provider.
    fn weapon_switch(&mut self, player: UnitId) -> Option<(u32, bool)>;
}

/// One message routed here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Run<'m> {
    pub player: UnitId,
    /// The drained message (its exact size).
    pub msg: &'m [u8],
    /// The result of the handler's own unit / point test (0x3D, 0x46,
    /// 0x47; 0: passed), made before the world call ([`handle`]).
    pub target: u32,
}

/// What the host answers about the player that the action wiring cannot
/// (the wired host's hireling list).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HostFacts {
    /// `Some`: the host's answer to [`PlayerWorld::hireling`].
    pub hireling: Option<Option<UnitId>>,
}

/// What a handler did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Outcome {
    /// The result code; `None`: the handler reached a call with no
    /// provider, the id stays a stub (nothing was changed).
    pub code: Option<u32>,
    /// 0x51: the slot and value to store.
    pub hotkey: Option<(usize, HotKey)>,
}

impl Outcome {
    fn code(c: u32) -> Self {
        Self {
            code: Some(c),
            hotkey: None,
        }
    }
    const STUB: Outcome = Outcome {
        code: None,
        hotkey: None,
    };
}

fn u16_at(m: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(m.get(at..at + 2)?.try_into().ok()?))
}

fn u32_at(m: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(m.get(at..at + 4)?.try_into().ok()?))
}

/// The exact size of each handled id (`client-messages.tsv`); 0x14 is
/// 4..=275 (§2.4 rule 1).
fn size_ok(id: u8, len: usize) -> bool {
    match id {
        0x14 => (4..=275).contains(&len),
        0x12 | 0x41 | 0x48 | 0x53 | 0x54 | 0x60 => len == 1,
        0x3F | 0x4D => len == 3,
        0x3D => len == 5,
        0x4B | 0x51 => len == 9,
        0x46 | 0x47 => len == 13,
        0x44 => len == 17,
        _ => false,
    }
}

/// The §9 handler of `r.msg[0]` (a [`Status::Handled`] id) on `w`.
pub fn run<W: PlayerWorld>(w: &mut W, r: &Run<'_>) -> Outcome {
    let (p, m) = (r.player, r.msg);
    let Some(&id) = m.first() else {
        return Outcome::code(3);
    };
    if !size_ok(id, m.len()) {
        return Outcome::code(3);
    }
    match id {
        0x12 => Outcome::code(end_inferno(w, p)),
        0x14 => Outcome::code(overhead_chat(w, p, m)),
        0x3D => Outcome::code(highlight_door(w, p, m, r.target)),
        0x3F => Outcome::code(play_audio(w, p, m)),
        0x41 => Outcome::code(resurrect(w, p)),
        0x44 => match staff_in_orifice(w, p, m) {
            Some(c) => Outcome::code(c),
            None => Outcome::STUB,
        },
        0x46 => Outcome::code(merc_interact(w, p, m, r.target)),
        0x47 => Outcome::code(move_merc(w, p, m, r.target)),
        0x48 => Outcome::code(turn_off_busy(w, p)),
        0x4B => Outcome::code(request_entity_update(w, p, m)),
        0x4D => Outcome::code(play_npc_message(w, p, m)),
        0x51 => bind_hotkey(w, p, m),
        0x53 => Outcome::code(stamina_on(w, p)),
        0x54 => Outcome::code(stamina_off(w, p)),
        0x60 => match swap_weapons(w, p) {
            Some(c) => Outcome::code(c),
            None => Outcome::STUB,
        },
        _ => Outcome::STUB,
    }
}

/// Rule 2 (`0x0054A260`): state 12 off with the update-queue insert; 0.
pub fn end_inferno<W: PlayerWorld>(w: &mut W, p: UnitId) -> u32 {
    w.set_state(p, STATE_INFERNO, false);
    0
}

/// Rule 3 (`0x0054A290`). The dispatcher's chat check (§2.4 rule 6)
/// already refused an empty or unterminated text and one of 256 or more
/// characters; the handler's own checks are kept in their order.
pub fn overhead_chat<W: PlayerWorld>(w: &mut W, p: UnitId, m: &[u8]) -> u32 {
    let body = &m[3..];
    let n = body.iter().position(|&c| c == 0).unwrap_or(body.len());
    if n == 0 {
        return 0;
    }
    if n >= 256 {
        return 2;
    }
    let text = &body[..n];
    // The name cstr after it is copied (≤ 16 bytes) and not used.
    if w.overhead_text_test(text) {
        return 0;
    }
    let frame = w.frame();
    let d = OVERHEAD_PER_CHAR * (n as i32).min(OVERHEAD_MAX_CHARS) + OVERHEAD_BASE;
    let end = frame.wrapping_add(d);
    w.replace_overhead(p, text, m[2], end);
    w.queue_update(p);
    w.or_flags(p, FLAG_OVERHEAD);
    w.schedule_event(p, EVENT_OVERHEAD, frame.wrapping_add(1).max(end));
    0
}

/// Rule 4 (`0x0054BF10`): the unit test (type 2, range 10, made by the
/// caller: `target`); else `0x005845D0`; 0.
pub fn highlight_door<W: PlayerWorld>(w: &mut W, p: UnitId, m: &[u8], target: u32) -> u32 {
    if target != 0 {
        return target;
    }
    let guid = u32_at(m, 1).unwrap_or(0);
    w.highlight_door(p, guid);
    0
}

/// Rule 5 (`0x0054C070`).
pub fn play_audio<W: PlayerWorld>(w: &mut W, p: UnitId, m: &[u8]) -> u32 {
    let sound = u16_at(m, 1).unwrap_or(0);
    if !AUDIO_SOUNDS.contains(&sound) {
        return 0;
    }
    if w.flags(p) & FLAG_NO_AUDIO != 0 {
        return 1;
    }
    w.sound(p, u32::from(sound), None);
    0
}

/// Rule 6 (`0x0054C0E0`), after the dead gate (§2.3 rule 3).
pub fn resurrect<W: PlayerWorld>(w: &mut W, p: UnitId) -> u32 {
    if w.mode(p) != MODE_DEAD {
        return 0;
    }
    if w.hardcore(p) {
        w.drop_client(p, DROP_HARDCORE);
        return 0;
    }
    // `0x0053FDF0` (§9 rule 6): its whole body is `return 0`; the value
    // is the warp's last argument.
    let warp_arg = 0;
    // `0x0056DFA0`: the passive states back on.
    for e in w.skill_entries(p) {
        let s = w.passive_state(e.skill);
        if let Ok(s) = u16::try_from(s) {
            if s > 0 {
                w.set_state(p, s, true);
                w.passive_state_apply(p, &e);
            }
        }
    }
    for stat in [STAT_LIFE, STAT_MANA, STAT_STAMINA] {
        let max = w.stat_max(p, stat);
        w.set_stat_send(p, stat, max);
    }
    w.or_flags(p, FLAG_RESURRECTED);
    w.set_state(p, STATE_UNINTERRUPTABLE, true);
    w.set_state(p, STATE_UNINTERRUPTABLE, false);
    let act = d2_sim::drlg::act_of_level(w.current_level(p));
    let town = d2_sim::drlg::ACT_BOUNDARIES[usize::from(act)];
    w.warp(p, town, warp_arg);
    w.start_mode_skip_gate(p, MODE_NEUTRAL);
    w.reselect_hand_skills(p);
    0
}

/// Rule 7 (`0x0054C380`): busy and trading → 3; else `0x00549520` and
/// its result (`None`: no provider).
pub fn staff_in_orifice<W: PlayerWorld>(w: &mut W, p: UnitId, m: &[u8]) -> Option<u32> {
    if w.busy(p) && w.trading(p) {
        return Some(3);
    }
    let object = u32_at(m, 5)?;
    let item = u32_at(m, 9)?;
    let action = u16_at(m, 13)?;
    w.staff_in_orifice(p, object, item, action)
}

/// `0x0054C430(merc, a, b, command)` (rule 8).
pub fn merc_command<W: PlayerWorld>(
    w: &mut W,
    p: UnitId,
    merc: u32,
    a: i32,
    b: i32,
    cmd: i32,
) -> u32 {
    let Some(m) = w.find_unit(UnitType::Monster, merc) else {
        return 1;
    };
    if w.hireling(p) != Some(m) {
        return 1;
    }
    // §9 rule 8: `0x0058EF40` copies five dwords; params 3 and 4 are
    // uninitialized stack bytes in 1.14d, and d2rs stores 0, 0.
    w.replace_ai_commands(m, [cmd, a, b, 0, 0]);
    w.sound(m, SOUND_MERC_ACK, Some(p));
    0
}

/// Rule 8, 0x46 (`0x0054C4B0`): type ≥ 6 → 2; the unit test (range 50,
/// `target`); then [`merc_command`]'s result.
pub fn merc_interact<W: PlayerWorld>(w: &mut W, p: UnitId, m: &[u8], target: u32) -> u32 {
    let (merc, guid, ty) = (
        u32_at(m, 1).unwrap_or(0),
        u32_at(m, 5).unwrap_or(0),
        u32_at(m, 9).unwrap_or(u32::MAX),
    );
    if ty >= 6 {
        return 2;
    }
    if target != 0 {
        return target;
    }
    merc_command(w, p, merc, guid as i32, ty as i32, COMMAND_INTERACT)
}

/// Rule 8, 0x47 (`0x0054C520`): the point test (range 50, `target`);
/// then [`merc_command`]; 0 whatever it returns.
pub fn move_merc<W: PlayerWorld>(w: &mut W, p: UnitId, m: &[u8], target: u32) -> u32 {
    if target != 0 {
        return target;
    }
    let merc = u32_at(m, 1).unwrap_or(0);
    let (x, y) = (u16_at(m, 5).unwrap_or(0), u16_at(m, 9).unwrap_or(0));
    merc_command(w, p, merc, i32::from(x), i32::from(y), COMMAND_MOVE);
    0
}

/// Rule 9 (`0x0054C590`).
pub fn turn_off_busy<W: PlayerWorld>(w: &mut W, p: UnitId) -> u32 {
    if !w.busy_flag(p) {
        return 2;
    }
    w.clear_busy_flag(p);
    0
}

/// Rule 10 (`0x0054C6D0`).
pub fn request_entity_update<W: PlayerWorld>(w: &mut W, p: UnitId, m: &[u8]) -> u32 {
    let ty = u32_at(m, 1).unwrap_or(u32::MAX);
    let Some(&uty) = UnitType::ALL.get(ty as usize).filter(|_| ty < 6) else {
        return 2;
    };
    let unit = w.find_unit(uty, u32_at(m, 5).unwrap_or(0));
    if ty == 0 && unit != Some(p) {
        return 3;
    }
    let Some(u) = unit else {
        return 1;
    };
    if !w.room_in_list_of(p, u) {
        return 1;
    }
    w.queue_update(u);
    w.or_flags_ex(u, FLAG_EX_RESEND);
    0
}

/// Rule 11 (`0x0054C690`).
pub fn play_npc_message<W: PlayerWorld>(w: &mut W, p: UnitId, m: &[u8]) -> u32 {
    let class = u16_at(m, 1).unwrap_or(u16::MAX);
    if u32::from(class) >= w.monstats_count() {
        return 2;
    }
    let d = w.difficulty();
    w.clear_npc_intro(p, d, class);
    0
}

/// Rule 12 (`0x0054C870`): the checks; the slot is stored by the caller
/// (client +0x3DC, [`SimGame::hotkeys`]); nothing is sent.
pub fn bind_hotkey<W: PlayerWorld>(w: &mut W, p: UnitId, m: &[u8]) -> Outcome {
    let Some(b) = crate::dispatch::bind_hotkey(m) else {
        return Outcome::code(3);
    };
    if usize::from(b.slot) >= HOTKEY_SLOTS {
        return Outcome::code(3);
    }
    let count = w.skill_count();
    let skill = u32::from(b.skill);
    let stored = if skill > count {
        -1
    } else if skill == count || !w.has_skill(p, i32::from(b.skill), b.item) {
        // skill = count, or 0 ≤ skill < count and the player lacks it.
        return Outcome::code(3);
    } else {
        b.skill as i16
    };
    Outcome {
        code: Some(0),
        hotkey: Some((
            usize::from(b.slot),
            HotKey {
                skill: stored,
                left: b.left,
                item: b.item,
            },
        )),
    }
}

/// Rule 13, 0x53 (`0x0054C940`).
pub fn stamina_on<W: PlayerWorld>(w: &mut W, p: UnitId) -> u32 {
    if w.mode(p) == MODE_WALK && w.stat(p, STAT_STAMINA) != 0 {
        w.set_mode(p, MODE_RUN);
    }
    0
}

/// Rule 13, 0x54 (`0x0054C990`).
pub fn stamina_off<W: PlayerWorld>(w: &mut W, p: UnitId) -> u32 {
    if w.mode(p) == MODE_RUN {
        w.set_mode(p, MODE_WALK);
    }
    0
}

/// Rule 14 (`0x0054CE70`): `None` when the switch has no provider.
pub fn swap_weapons<W: PlayerWorld>(w: &mut W, p: UnitId) -> Option<u32> {
    if !w.expansion() {
        return Some(3);
    }
    if w.has_used_skill(p) || w.is_dead(p) {
        return Some(0);
    }
    let (r, fail) = w.weapon_switch(p)?;
    Some(if r == 0 && fail { 3 } else { 0 })
}

/// The unit test `0x00548F80` with range `range` on the staged facts
/// (§2.4 rule 4): missing → 1; an item the player owns → 0; another act
/// → 2; else the Chebyshev test (→ 1).
pub fn unit_code(t: UnitTarget, range: u32) -> u32 {
    match t {
        UnitTarget::Missing => 1,
        UnitTarget::OwnedItem => 0,
        UnitTarget::OtherAct => 2,
        UnitTarget::At { player, target } => u32::from(!within(player, target, range)),
    }
}

/// The point test `0x00548EF0` with range `range` (§2.4 rule 3): 1 out of
/// range. A player without a staged position fails it.
pub fn point_code(player: Option<Pos>, target: Pos, range: u32) -> u32 {
    match player {
        Some(p) => u32::from(!within(p, target, range)),
        None => 1,
    }
}

fn within(a: Pos, b: Pos, r: u32) -> bool {
    a.x.abs_diff(b.x) <= r && a.y.abs_diff(b.y) <= r
}

/// The handler's own target test, from the staged facts.
fn target_code<D: EventDispatch, W: WorldHost<D>>(
    sim: &SimGame<D, W>,
    client: ClientId,
    msg: &[u8],
) -> u32 {
    match msg[0] {
        0x3D => unit_code(
            sim.unit_target(client, 2, u32_at(msg, 1).unwrap_or(0)),
            DOOR_RANGE,
        ),
        0x46 => {
            let ty = u32_at(msg, 9).unwrap_or(u32::MAX);
            if ty >= 6 {
                // The handler refuses the type first (2).
                return 0;
            }
            unit_code(
                sim.unit_target(client, ty, u32_at(msg, 5).unwrap_or(0)),
                MERC_RANGE,
            )
        }
        0x47 => {
            let target = Pos {
                x: i32::from(u16_at(msg, 5).unwrap_or(0)),
                y: i32::from(u16_at(msg, 9).unwrap_or(0)),
            };
            point_code(sim.player_pos(client), target, MERC_RANGE)
        }
        _ => 0,
    }
}

/// The 0x15 resync of a client whose point target was out of range
/// (`intents-events.md` §2.4 rule 3): the player is queued for update
/// with flag-ex bit 0x10000, which makes its next update send S→C 0x15
/// (`pathing.md` §10 rule 2), exactly as C→S 0x4B for the player's own
/// unit does (§9 rule 10). A host without the player provider does
/// nothing.
pub fn resync<D: EventDispatch, W: WorldHost<D>>(sim: &mut SimGame<D, W>, client: ClientId) {
    let Some(player) = sim.player_of(client) else {
        return;
    };
    let Some(guid) = sim.game.lists.unit(player).map(|e| e.guid) else {
        return;
    };
    let mut msg = [0u8; 9];
    msg[0] = 0x4B;
    msg[5..9].copy_from_slice(&guid.to_le_bytes());
    let run = Run {
        player,
        msg: &msg,
        target: 0,
    };
    let (game, events) = (&mut sim.game, &mut sim.events);
    let _ = sim.world.player(game, events, run);
}

/// The §9 handler for one dispatched message. `None`: not an id routed
/// here, no player, the host has no [`PlayerWorld`] provider, or the
/// handler reached a call with no provider: the caller keeps its stub.
pub fn handle<D: EventDispatch, W: WorldHost<D>>(
    sim: &mut SimGame<D, W>,
    client: ClientId,
    msg: &[u8],
    size: usize,
    out: &mut dyn MessageSink,
) -> Option<ResultCode> {
    let id = *msg.first()?;
    if !handled(id) {
        return None;
    }
    let msg = &msg[..size.min(msg.len())];
    let player = sim.player_of(client)?;
    let target = if size_ok(id, msg.len()) {
        target_code(sim, client, msg)
    } else {
        0
    };
    let run = Run {
        player,
        msg,
        target,
    };
    let (game, events) = (&mut sim.game, &mut sim.events);
    let outcome = sim.world.player(game, events, run)?;
    let mut faults = Vec::new();
    for (unit, bytes) in sim.world.take_sent(&mut sim.events) {
        // §3.2 rule 1: a player without a client receives nothing.
        if let Some(c) = sim.client_of(unit) {
            if let Err(e) = out.queue(c, &bytes) {
                faults.push(WorldError::from(e));
            }
        }
    }
    let code = outcome.code?;
    let code = match outcome.hotkey {
        Some((slot, key)) => match sim.client_of(player) {
            Some(c) => {
                sim.set_hotkey(c, slot, key);
                code
            }
            // `0x005356B0`: no client → 3.
            None => 3,
        },
        None => code,
    };
    if !faults.is_empty() {
        for error in faults {
            sim.world.fault(WorldFault { client, id, error });
        }
        return Some(ResultCode::Malformed);
    }
    Some(super::skills::code(code as i32))
}
