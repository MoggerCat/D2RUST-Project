// Spec: specs/audio/triggers.md §2 (server sound events), §3 (player event sounds)
//! S→C 0x2C PlaySound events (`0x004CBDE0`) and the player event sounds
//! `0x004CB9C0(U, e)`. Non-audio effects (overhead text, quest stingers,
//! the stinger re-arm of `audio/environment.md` §3 r5) are returned as
//! [`Followup`]s for their owners, in the order they happen.

use super::movement::flee;
use super::npc::{greet, GreetMode, NpcGreeting};
use super::{class_record, sid, Ctx, TriggerError, Unit, UnitSound, MONSTER, PLAYER};
use crate::bridge::world::UnitKey;

/// A non-audio effect of an event rule, for its owner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Followup {
    /// Overhead text of this sound id (`0x004A0200`, `client/ui.md`).
    OverheadText(i32),
    /// Quest line held by a stinger (`audio/environment.md` §3): event e,
    /// line id.
    QuestStinger { event: u16, id: i32 },
    /// Event 92: stinger speech re-arm `0x004DCE10(25, 1)`
    /// (`audio/environment.md` §3 r5).
    StingerRearm,
}

/// Class 271 (`roguehire`, §2 r2 events 15, 84–87).
pub const ROGUEHIRE: i32 = 271;

/// Inputs of one 0x2C event beyond the unit itself.
pub struct EventExtra<'a> {
    /// Local player P (`[0x007A6A70]`); `None` when missing.
    pub local: Option<UnitKey>,
    /// Event 12: the `stsound` the caller read from U.
    /// TODO(spec: audio/triggers.md open question 4): which skill/record
    /// `0x006256B0(U, 0x44)` and `0x00625D00(·, 350, 0)` select.
    pub event12_stsound: i32,
    /// Event 18: U's NPC greeting record and state (§10 r1); `None` = no
    /// record (id 0).
    pub greeting: Option<&'a mut NpcGreeting>,
    /// Day phase for the greeting's time line (`audio/environment.md` §1 r3).
    pub day_phase: u8,
}

/// S→C 0x2C event `event` on unit U (§2 r2, r3). The caller looks U up
/// (`0x00463990`); a missing unit makes no call.
pub fn server_event(
    cx: &mut Ctx,
    u: &Unit,
    us: &mut UnitSound,
    event: u16,
    extra: EventExtra,
) -> Result<Vec<Followup>, TriggerError> {
    let on_u = Some(u.key);
    let rogue = u.class == ROGUEHIRE;
    let mut out = Vec::new();
    match event {
        10 => {
            let h = cx.req(2673, on_u, 0);
            cx.s.set_volume(h, 120);
        }
        12 => {
            if extra.event12_stsound > 0 {
                cx.req(extra.event12_stsound, on_u, 0);
            }
        }
        13 => {
            cx.req(2634, on_u, 0);
        }
        14 => {
            cx.req(2635, on_u, 0);
        }
        15 => {
            if rogue {
                cx.req(4290, on_u, 0);
            }
        }
        16 => {
            let id = match (u.unit_type(), u.monsounds) {
                (MONSTER, Some(r)) => sid(r.taunt),
                _ => 0,
            };
            cx.req(id, on_u, 0);
        }
        17 => flee(cx, u, us),
        18 => {
            let pick = match extra.greeting {
                Some(g) => greet(cx, g, GreetMode::Event, extra.day_phase),
                None => 0,
            };
            cx.s.request(pick, extra.local, 0, 1, 0);
        }
        84..=87 => {
            let (female, male) = match event {
                84 => (4615, 4625),
                85 => (4612, 4616),
                86 => (4613, 4619),
                _ => (4614, 4622),
            };
            let id = if rogue { female } else { male };
            out.push(Followup::OverheadText(id));
            cx.req(id, on_u, 0);
            if u.unit_type() == PLAYER {
                out.extend(player_event(cx, u, event)?);
            }
        }
        90 => {
            cx.req(4379, on_u, 0);
        }
        91 => {
            cx.req(8, extra.local, 0);
        }
        92 => out.push(Followup::StingerRearm),
        93 => {
            cx.req(2553, on_u, 0);
        }
        _ => {
            if u.unit_type() == PLAYER {
                out.extend(player_event(cx, u, event)?);
            }
        }
    }
    Ok(out)
}

/// §3 r4: quest events held by a stinger.
pub const QUEST_STINGER_EVENTS: [u16; 11] = [33, 34, 35, 37, 50, 52, 66, 75, 80, 82, 83];
/// §3 r4: quest events requested with delay 0 (others 12; `0x004CBD68`).
pub const QUEST_DELAY0_EVENTS: [u16; 10] = [38, 40, 41, 42, 43, 44, 46, 47, 55, 56];
/// §3 r4: delay of the other quest lines.
pub const QUEST_DELAY: u32 = 12;
/// §3 r6: repeat window of the global speech guard (C).
pub const SPEECH_REPEAT: u32 = 75;

/// Fixed player events 1–11 (§3 r5 table `0x004CBD7C`): (e, id, on U).
pub const FIXED_EVENTS: [(u16, i32, bool); 9] = [
    (1, 235, true),
    (2, 7, false),
    (3, 10, false),
    (4, 11, false),
    (5, 12, false),
    (6, 13, false),
    (7, 2230, true),
    (8, 2231, true),
    (9, 9, true),
];
/// Event 11: 228 `item_key_used` on U.
pub const EVENT_KEY_USED: (u16, i32) = (11, 228);

/// Player event sound `0x004CB9C0(U, e)` (§3). U is a player; its class
/// is `u.class`.
pub fn player_event(cx: &mut Ctx, u: &Unit, e: u16) -> Result<Vec<Followup>, TriggerError> {
    // r2. TODO(spec: audio/triggers.md §3 r2): "P is missing" is not
    // representable here (U is a unit); only the dead check applies.
    if u.is_local && u.mode == 17 {
        return Ok(Vec::new());
    }
    let rec = class_record(u.class)?;
    let on_u = Some(u.key);
    let mut out = Vec::new();
    let requested = match e {
        25..=32 => {
            if cx.s.speaking(u.key) {
                None
            } else {
                let id = rec.chat_base + 2 * (e as i32 - 25);
                cx.req(id, on_u, 0);
                Some(id)
            }
        }
        33..=83 => {
            let id = rec.quest_base + (e as i32 - 33);
            if QUEST_STINGER_EVENTS.contains(&e) {
                out.push(Followup::QuestStinger { event: e, id });
                None
            } else {
                let d = if QUEST_DELAY0_EVENTS.contains(&e) {
                    0
                } else {
                    QUEST_DELAY
                };
                cx.req(id, on_u, d);
                Some(id)
            }
        }
        19..=24 => {
            let id = match e {
                19 => rec.impossible,
                20 => rec.cantuseyet,
                21 => rec.needmana,
                22 => rec.needkey,
                23 => rec.cantcarry,
                _ => rec.notintown,
            };
            // r6: the global speech guard.
            if cx.s.speaking(u.key)
                || (id == cx.g.speech_id && cx.c.wrapping_sub(cx.g.speech_time) < SPEECH_REPEAT)
            {
                None
            } else {
                cx.g.speech_time = cx.c;
                cx.g.speech_id = id;
                cx.req(id, on_u, 0);
                Some(id)
            }
        }
        _ => {
            if let Some(&(_, id, unit)) = FIXED_EVENTS.iter().find(|f| f.0 == e) {
                cx.req(id, if unit { on_u } else { None }, 0);
                Some(id)
            } else if e == EVENT_KEY_USED.0 {
                cx.req(EVENT_KEY_USED.1, on_u, 0);
                Some(EVENT_KEY_USED.1)
            } else {
                None
            }
        }
    };
    // r7.
    if let Some(id) = requested {
        out.push(Followup::OverheadText(id));
    }
    Ok(out)
}
