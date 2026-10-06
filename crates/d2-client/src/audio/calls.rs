// Spec: specs/audio/sound-table.md (§5 requests, §4 r5 client RNG)
// Spec: specs/audio/triggers.md (§1 conventions: request, volume set, detach, group stops, speaking)
//! The call surface the trigger rules (`audio/triggers.md`) and the
//! environment state machines (`audio/environment.md`) use to reach the
//! sound layer (`audio/sound-table.md`). The rules only say *when* and
//! *with which arguments* a call is made; what the call does is the sound
//! table's (implemented by [`super::sound_table`]). Keeping the calls behind
//! a trait lets each rule set be tested against a recording fake.

use crate::bridge::world::UnitKey;

/// A request handle (`sound-table.md` §5 r3: +0x08, from a pre-incremented
/// counter). `0` means "no request" (the original's return value 0).
pub type Handle = u32;

/// Request flag bit 0: exact id, no variant pick (`sound-table.md` §5 r3).
pub const FLAG_EXACT: u32 = 1;
/// Request flag bit 1: no fade-in (`sound-table.md` §5 r3).
pub const FLAG_NO_FADE_IN: u32 = 2;

/// The sound layer as the rule functions call it. Method names follow
/// `audio/triggers.md` §1; each names its 1.14d entry point.
pub trait SoundCalls {
    /// `0x004B9A00(id, unit, delay, flags, offset)` (`triggers.md` §1 r1,
    /// `sound-table.md` §5 r1–r4). `delay` is in sound ticks (T). Returns
    /// the handle, or 0 when no request is made.
    fn request(
        &mut self,
        id: i32,
        unit: Option<UnitKey>,
        delay: u32,
        flags: u32,
        offset: u32,
    ) -> Handle;

    /// `0x004B9B50(h, v)` (`triggers.md` §1 r2): volume (+0x1C) := v if
    /// the handle exists.
    fn set_volume(&mut self, h: Handle, v: i32);

    /// `0x004B9EF0(h, target, delay, len)` (`sound-table.md` §5 r5).
    fn fade(&mut self, h: Handle, target: i32, delay: u32, len: u32);

    /// `0x004BA790(h, unit, force)` (`triggers.md` §1 r3).
    fn detach(&mut self, h: Handle, unit: UnitKey, force: bool);

    /// `0x004BA840(h)` (`triggers.md` §1 r4).
    fn stop_handle(&mut self, h: Handle);
    /// `0x004BA890(id)`: every request of that id.
    fn stop_id(&mut self, id: i32);
    /// `0x004BA8F0`: every request in the song range 4,657–4,684.
    fn stop_songs(&mut self);
    /// `0x004BA950(a, b)`: ids 52–71 except group bases `a`, `b`.
    fn stop_52_71_except(&mut self, a: i32, b: i32);
    /// `0x004BA9D0(a, b)`: ids 72–201 except group bases `a`, `b`.
    fn stop_72_201_except(&mut self, a: i32, b: i32);
    /// `0x004BAA50`: ids 2,934–4,656 (speech).
    fn stop_speech(&mut self);

    /// `speaking(U)` (`triggers.md` §1 r8, `0x004B9E90` → `0x004B9700`).
    fn speaking(&self, unit: UnitKey) -> bool;
    /// `any_speech` (`triggers.md` §1 r8, `0x004B9C20`).
    fn any_speech(&self) -> bool;
    /// Whether a request with this handle exists and has not ended.
    fn is_active(&self, h: Handle) -> bool;

    /// The sound tick T (`[0x007BC9BC]`, `sound-table.md` §6.1).
    fn sound_tick(&self) -> u32;

    /// `roll(n)` on the local player's client unit seed (`sound-table.md`
    /// §4 r5, `0x004E40A0`; `sim/rng.md` §3 `roll`).
    fn roll(&mut self, n: i32) -> u32;
}

/// `uniform(lo, hi)` = lo + roll(hi − lo + 1) (`triggers.md` §1 r7,
/// `0x004E4100`).
pub fn uniform(s: &mut dyn SoundCalls, lo: i32, hi: i32) -> i32 {
    lo.wrapping_add(s.roll(hi.wrapping_sub(lo).wrapping_add(1)) as i32)
}

/// `jitter(r)` = roll(2r + 1) − r (`triggers.md` §1 r7, `0x004E4120`).
pub fn jitter(s: &mut dyn SoundCalls, r: i32) -> i32 {
    (s.roll(r.wrapping_mul(2).wrapping_add(1)) as i32).wrapping_sub(r)
}
