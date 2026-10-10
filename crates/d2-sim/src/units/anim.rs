// Spec: specs/sim/units.md §4.2, §4.4; specs/skills/sequences.md §3
//! Mode schedules: the animation schedule of events 0 and 1 (§4.2, the
//! main form `0x005539B0` and the variants `0x00553B10`, `0x00553C70`,
//! `0x00553DC0`) as a pure function of its inputs, and its application
//! to the timer queue; the every-tick movement event (§4.4); the
//! sequence branch of the frame advance `0x00623E00`
//! ([`advance_sequence`]) and its plain branch ([`advance_plain`]).

use thiserror::Error;

use crate::game::{Game, GameError};
use crate::tick::events::event;

use super::record::{Anim, ANIM_EVENTS};
use super::UnitId;

/// One schedule of §4.2: event type, expire, a1, a2.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Scheduled {
    pub event: u8,
    pub expire: i32,
    pub a1: u32,
    pub a2: u32,
}

/// Which §4.2 form runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Form {
    /// `0x005539B0`, start index = the frame bonus `0x00623B10`.
    Main { bonus: i32 },
    /// `0x00553B10`, argument a percent.
    Percent(i32),
    /// `0x00553C70`, argument a frame count (p ≤ 0: nothing at all).
    Frames(i32),
    /// `0x00553DC0`, argument a start frame.
    StartFrame(i32),
}

/// A computed schedule.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Schedule {
    /// The variants cancel the unit's type-0 and type-1 events first.
    pub cancels: bool,
    pub events: Vec<Scheduled>,
    /// The new current frame +0x44, when the form sets it.
    pub frame: Option<i32>,
}

/// Inputs §4.2 cannot proceed with.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum AnimError {
    /// A negative speed never ends the loop in 1.14d (edge case 3).
    #[error("negative animation speed {0}: the 1.14d loop never ends")]
    NegativeSpeed(i32),
    /// The frame position `a += s` passes 2^31 − 1 before reaching the
    /// frame count: the signed 1.14d loop wraps and never ends, as with a
    /// negative speed (edge case 3).
    #[error(
        "animation speed {speed} wraps before frame count {frame_count}: the 1.14d loop never ends"
    )]
    Endless { speed: i32, frame_count: i32 },
    /// A variant read an event byte outside the AnimData record
    /// (index ≥ 144, or < −1): the record bytes past it are not known
    /// here. TODO(units.md edge case 1): reproduce from the record set.
    #[error("event byte index {0} outside the animation record")]
    EventIndex(i32),
    /// No animation data to schedule from.
    #[error("no animation record")]
    NoRecord,
    #[error(transparent)]
    Game(#[from] GameError),
}

/// Where the event bytes come from: the AnimData record (index −1 is
/// +0x0F), or a sequence (`0x006634C0`).
#[derive(Clone, Copy, Debug)]
pub enum Events<'a> {
    Record {
        byte_0f: u8,
        events: &'a [u8; ANIM_EVENTS],
    },
    Sequence(&'a [u8]),
}

impl Events<'_> {
    fn get(&self, i: i32) -> Result<u8, AnimError> {
        match *self {
            Events::Record { byte_0f, events } => match i {
                -1 => Ok(byte_0f),
                _ => usize::try_from(i)
                    .ok()
                    .and_then(|i| events.get(i).copied())
                    .ok_or(AnimError::EventIndex(i)),
            },
            // TODO(sequence spec): sequence event bytes outside the
            // sequence are not described.
            Events::Sequence(e) => usize::try_from(i)
                .ok()
                .and_then(|i| e.get(i).copied())
                .ok_or(AnimError::EventIndex(i)),
        }
    }
}

/// §4.2: the schedule of `form` at frame `f` with speed `s`, frame
/// count `frame_count` (8.8) and current frame `cur` (+0x44, 8.8).
pub fn schedule(
    form: Form,
    f: i32,
    s: i32,
    frame_count: i32,
    cur: i32,
    events: Events<'_>,
) -> Result<Option<Schedule>, AnimError> {
    let variant = !matches!(form, Form::Main { .. });
    if let Form::Frames(p) = form {
        if p <= 0 {
            return Ok(None);
        }
    }
    // TODO(units.md §4.2): "speed 0 gives event 1 at f + 1 in all
    // forms"; the variants' +0x44 write is read as skipped then, as in
    // the main form.
    if s == 0 {
        return Ok(Some(Schedule {
            cancels: variant,
            events: vec![Scheduled {
                event: event::END_ANIM,
                expire: f.wrapping_add(1),
                a1: 0,
                a2: 0,
            }],
            frame: None,
        }));
    }
    if s < 0 {
        return Err(AnimError::NegativeSpeed(s));
    }
    let cur_frame = cur >> 8;
    let (start, c) = match form {
        Form::Main { bonus } => (bonus, bonus),
        Form::Percent(p) => {
            let c = 100i32
                .wrapping_sub(p)
                .wrapping_mul(f.wrapping_sub(cur_frame))
                / 100;
            (c.wrapping_sub(1), c)
        }
        Form::Frames(p) => {
            let c = f.wrapping_sub(cur_frame).wrapping_sub(p);
            (c.wrapping_sub(1), c)
        }
        Form::StartFrame(p) => (p.wrapping_sub(1), p),
    };
    let mut n = f;
    let mut i = start;
    let mut k = 0u32;
    let mut a = c.wrapping_mul(256).wrapping_add(s);
    if a < frame_count {
        // The last a + s (i64) must stay a valid i32, else the loop wraps.
        let (a0, s64, fc) = (i64::from(a), i64::from(s), i64::from(frame_count));
        let iterations = (fc - a0 + s64 - 1) / s64;
        if a0 + iterations * s64 > i64::from(i32::MAX) {
            return Err(AnimError::Endless {
                speed: s,
                frame_count,
            });
        }
    }
    let mut out = Vec::new();
    while a < frame_count {
        n = n.wrapping_add(1);
        while i <= a >> 8 && (variant || i < ANIM_EVENTS as i32) {
            let e = events.get(i)?;
            if variant {
                if matches!(e, 1..=4) {
                    out.push(Scheduled {
                        event: event::MODE_CHANGE,
                        expire: n,
                        a1: u32::from(e),
                        a2: 0,
                    });
                }
            } else if matches!(e, 1 | 2 | 4) {
                out.push(Scheduled {
                    event: event::MODE_CHANGE,
                    expire: n,
                    a1: u32::from(e),
                    a2: k,
                });
                k += 1;
            } else if e == 3 {
                out.push(Scheduled {
                    event: event::MODE_CHANGE,
                    expire: n,
                    a1: 3,
                    a2: 0,
                });
            }
            i += 1;
        }
        a = a.wrapping_add(s);
    }
    if n == f {
        n = f.wrapping_add(1);
    }
    out.push(Scheduled {
        event: event::END_ANIM,
        expire: n.wrapping_add(1),
        a1: 0,
        a2: 0,
    });
    let frame = if variant {
        f.wrapping_sub(c).wrapping_mul(256)
    } else {
        f.wrapping_mul(256)
    };
    Ok(Some(Schedule {
        cancels: variant,
        events: out,
        frame: Some(frame),
    }))
}

/// `0x00553990`: cancel the unit's events of type 0, then type 1, any
/// argument.
pub fn cancel_mode_events(game: &mut Game, unit: UnitId) {
    game.timers
        .cancel_unit_events(unit, event::MODE_CHANGE, None);
    game.timers.cancel_unit_events(unit, event::END_ANIM, None);
}

/// Runs §4.2 for a unit's animation fields: the speed and frame count
/// of its sequence if it has one, else +0x4C / +0x48; event bytes from
/// the sequence or the AnimData record. Cancels (variants), schedules,
/// then sets +0x44.
pub fn run(game: &mut Game, unit: UnitId, anim: &mut Anim, form: Form) -> Result<(), AnimError> {
    let f = game.frame;
    let sched = match &anim.sequence {
        Some(seq) => schedule(
            form,
            f,
            seq.speed,
            seq.frame_count,
            anim.frame,
            Events::Sequence(&seq.events),
        )?,
        None => {
            let rec = anim.record.as_ref().ok_or(AnimError::NoRecord)?;
            schedule(
                form,
                f,
                i32::from(anim.speed),
                anim.frame_count,
                anim.frame,
                Events::Record {
                    byte_0f: rec.byte_0f,
                    events: &rec.events,
                },
            )?
        }
    };
    let Some(sched) = sched else {
        return Ok(());
    };
    apply(game, unit, &sched)?;
    if let Some(frame) = sched.frame {
        anim.frame = frame;
    }
    Ok(())
}

/// Applies a schedule to the timer queue.
pub fn apply(game: &mut Game, unit: UnitId, sched: &Schedule) -> Result<(), GameError> {
    if sched.cancels {
        cancel_mode_events(game, unit);
    }
    for e in &sched.events {
        game.schedule_event(unit, u32::from(e.event), e.expire, None, e.a1, e.a2)?;
    }
    Ok(())
}

/// `0x00553F00` (§4.4): cancel the unit's type-0 events, then schedule
/// an every-tick type-0 event, args (0, 0).
pub fn every_tick_movement(game: &mut Game, unit: UnitId) -> Result<(), GameError> {
    game.timers
        .cancel_unit_events(unit, event::MODE_CHANGE, None);
    let owner = game.owner(unit)?;
    game.timers
        .schedule_every_tick(owner, u32::from(event::MODE_CHANGE), None, 0, 0)
        .map_err(GameError::from)?;
    Ok(())
}

/// The sequence branch of the frame advance `0x00623E00`
/// (`skills/sequences.md` §3): +0x4E := 0; p = +0x38 + +0x3C, minus
/// +0x34 once when ≥ +0x34; +0x38 := p; +0x48 −= +0x3C; then the frame
/// setup with b = the old +0x38 stores in +0x4E the event byte of frame
/// p (p = b), else the last non-zero event byte of frames (b >> 8) + 1 …
/// p >> 8 (0 when none or the range is empty). False (nothing done)
/// without a sequence. The drawn mode (+0x40) is not kept; the drawn frame
/// (+0x44) is, when the sequence holds the drawn frames.
pub fn advance_sequence(anim: &mut Anim) -> bool {
    let Some(seq) = anim.sequence.as_mut() else {
        return false;
    };
    let old = seq.pos;
    let mut p = old.wrapping_add(seq.speed);
    if p >= seq.frame_count {
        p = p.wrapping_sub(seq.frame_count);
    }
    seq.pos = p;
    anim.frame_count = anim.frame_count.wrapping_sub(seq.speed);
    let byte = |i: i32| {
        usize::try_from(i)
            .ok()
            .and_then(|i| seq.events.get(i))
            .copied()
            .unwrap_or(0)
    };
    let (a, b) = (p >> 8, old >> 8);
    // Frame setup `0x00621210`: +0x44 := the drawn frame of frame p · 256
    // (`skills/sequences.md` §3).
    if let Some(&d) = usize::try_from(p >> 8).ok().and_then(|i| seq.drawn.get(i)) {
        anim.frame = i32::from(d) << 8;
    }
    anim.action_frame = if p == old {
        byte(a)
    } else {
        (b + 1..=a).rev().map(byte).find(|&e| e != 0).unwrap_or(0)
    };
    true
}

/// The plain branch of the frame advance `0x00623E00` (`units.md` §4.2
/// "Frame advance and +0x4E", rule 2; REC-2065): +0x4E := 0; j = the
/// current frame >> 8 (+ 1 when the speed is at least 256); the frame
/// grows by the speed; while it is at or past the count F, the action
/// bytes of the frames j .. F >> 8 are crossed (the last byte 1–4 wins),
/// F is subtracted, j := 0 and the frame bonus b · 256 is added; then
/// the bytes of j ..= frame >> 8 are crossed. Bytes past the 144 the
/// record holds are not read. False (nothing done) with a sequence.
pub fn advance_plain(anim: &mut Anim, bonus: i32) -> bool {
    if anim.sequence.is_some() {
        return false;
    }
    anim.action_frame = 0;
    let events: &[u8] = anim.record.as_ref().map_or(&[], |r| &r.events[..]);
    let mut seen = 0u8;
    let mut cross = |from: i32, to_excl: i32| {
        let mut j = from.max(0);
        while j < to_excl && (j as usize) < ANIM_EVENTS {
            if let Some(&e) = events.get(j as usize) {
                if (1..=4).contains(&e) {
                    seen = e;
                }
            }
            j += 1;
        }
    };
    let speed = i32::from(anim.speed);
    let mut j = (anim.frame >> 8) + i32::from(speed >= 256);
    let mut cur = anim.frame.wrapping_add(speed);
    let count = anim.frame_count;
    while cur >= count {
        cross(j, count >> 8);
        cur = cur.wrapping_sub(count);
        j = 0;
        cur = cur.wrapping_add(bonus << 8);
        if count <= 0 {
            break;
        }
    }
    cross(j, (cur >> 8) + 1);
    anim.frame = cur;
    anim.action_frame = seen;
    true
}

#[cfg(test)]
mod plain_advance_tests {
    use super::*;
    use crate::units::record::AnimRecord;

    fn anim(frame: i32, count: i32, speed: i16, events: &[(usize, u8)]) -> Anim {
        let mut ev = [0u8; ANIM_EVENTS];
        for &(i, e) in events {
            ev[i] = e;
        }
        Anim {
            frame,
            frame_count: count,
            speed,
            record: Some(AnimRecord {
                frames: (count >> 8) as u32,
                byte_0f: 0,
                events: ev,
            }),
            ..Anim::default()
        }
    }

    // 1.14d gen-mon-507 (bloodlord2 A2): 7936 + 208 wraps over 5120 to 3024.
    #[test]
    fn wraps_past_the_count() {
        let mut a = anim(7936, 5120, 208, &[(15, 1)]);
        assert!(advance_plain(&mut a, 0));
        assert_eq!(a.frame, 3024);
        assert_eq!(a.action_frame, 0);
    }

    #[test]
    fn last_action_byte_crossed_wins() {
        let mut a = anim(256 * 5, 5120, 256 * 3, &[(6, 1), (7, 2), (9, 4)]);
        assert!(advance_plain(&mut a, 0));
        assert_eq!(a.frame, 256 * 8);
        assert_eq!(a.action_frame, 2);
    }
}
