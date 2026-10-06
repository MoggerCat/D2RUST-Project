// Spec: specs/audio/triggers.md §10 (NPC speech)
// Spec: specs/audio/npc-speech.tsv
//! NPC greetings `0x004E0590`, dialog lines `0x004A10E0` and the NPC
//! Speech option `0x0047CEF0`.

use super::tables::NpcSpeech;
use super::{detach_skill_voices, Ctx, Unit};
use crate::audio::calls::{Handle, FLAG_EXACT};
use crate::bridge::world::UnitKey;

/// One NPC greeting record (`0x004E0370`, 6 dwords: greet, inactive,
/// time, return, last, tick).
///
/// TODO(spec: audio/triggers.md §10 r1): the class → record table (35
/// classes, 28 records at `0x0072AE28`–`0x0072B0C8`) is not in the spec as
/// data; the caller supplies the record.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NpcGreeting {
    pub greet: i32,
    pub inactive: i32,
    pub time: i32,
    pub ret: i32,
    /// Last pick.
    pub last: i32,
    /// C of the last pick.
    pub tick: u32,
}

/// Greeting attempts (§10 r1).
pub const GREET_ATTEMPTS: u32 = 20;

/// The greeting mode argument of `0x004E0590`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GreetMode {
    /// Mode 0: may pick the inactive line.
    Idle,
    /// Mode 1 (event 18).
    Event,
    /// Mode 2: the return line.
    Return,
}

/// Greeting pick `0x004E0590(N, mode)` (§10 r1). Returns the id the
/// caller requests with flags 1 (exact).
///
/// TODO(spec: audio/triggers.md §10 r1): whether mode 2 also updates
/// last / tick is not stated; d2rs updates them only after the attempts.
pub fn greet(cx: &mut Ctx, g: &mut NpcGreeting, mode: GreetMode, day_phase: u8) -> i32 {
    if mode == GreetMode::Return {
        return g.ret;
    }
    let mut pick = 0;
    for _ in 0..GREET_ATTEMPTS {
        // 1.
        let mut s = g.greet;
        if mode == GreetMode::Idle && g.inactive != 0 && cx.s.roll(2) == 0 {
            s = g.inactive;
        }
        // 2.
        if g.time != 0 && (s == 0 || cx.s.roll(3) == 0) {
            s = g.time
                + match day_phase {
                    1 => 0,
                    2 | 3 => 1,
                    _ => 2,
                };
        }
        // 3, 4.
        pick = cx.s.variant(s);
        if pick != g.last {
            break;
        }
    }
    g.last = pick;
    g.tick = cx.c;
    pick
}

/// Greeting from NPC interaction (`0x004B4FD0`, `0x004B66B0`): detach N's
/// skill voices with force, pick, request on P with flags 1. Returns the
/// handle (`0x004B66B0` keeps it in `[0x007C0DB8]`).
pub fn interact_greeting(
    cx: &mut Ctx,
    g: &mut NpcGreeting,
    npc: &Unit,
    local: Option<UnitKey>,
    mode: GreetMode,
    day_phase: u8,
) -> Handle {
    detach_skill_voices(cx.s, npc.key, npc.monsounds);
    let pick = greet(cx, g, mode, day_phase);
    cx.s.request(pick, local, 0, FLAG_EXACT, 0)
}

/// Dialog line state: `[0x0072AE24]` and the remembered line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DialogState {
    /// NPC speech enabled (`[0x0072AE24]`, starts 1; open question 7).
    pub enabled: bool,
    pub handle: Handle,
    pub id: i32,
}

impl Default for DialogState {
    fn default() -> Self {
        DialogState {
            enabled: true,
            handle: 0,
            id: 0,
        }
    }
}

/// Dialog line delay (T) and the fade of the previous line (§10 r2).
pub const DIALOG_DELAY: u32 = 5;
pub const DIALOG_FADE: u32 = 4;

/// Dialog line `0x004A10E0(N, key)` (§10 r2). "Playing" for the previous
/// line is read as [`SoundCalls::is_active`](crate::audio::calls::SoundCalls::is_active).
pub fn dialog_line(
    cx: &mut Ctx,
    st: &mut DialogState,
    table: &NpcSpeech,
    npc: &Unit,
    local: Option<UnitKey>,
    key: i32,
) {
    if st.handle != 0 && cx.s.is_active(st.handle) {
        cx.s.fade(st.handle, 0, 0, DIALOG_FADE);
    }
    cx.s.stop_speech();
    if !st.enabled {
        return;
    }
    let s = table.sound(key);
    if s != 0 {
        detach_skill_voices(cx.s, npc.key, npc.monsounds);
        st.handle = cx.req(s, local, DIALOG_DELAY);
        st.id = s;
    }
}

/// NPC Speech option (`0x0047CEF0`, §10 r3): 0 → enabled (text off),
/// 1 → disabled (text on), 2 → both on. Returns whether text is shown.
pub fn set_npc_speech_option(st: &mut DialogState, value: u8) -> bool {
    st.enabled = value != 1;
    value != 0
}

/// §10 r4 fixed NPC lines (conditions: open question 8).
pub const WUSSIE_CHEER: i32 = 4603;
pub const WUSSIE_HELP_ME: i32 = 4607;
pub const WUSSIE_CLASS: i32 = 534;
pub const GUARD_HALT: i32 = 3983;
pub const NIHLATHAK_HURRYUP: i32 = 4560;
