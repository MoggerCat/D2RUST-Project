// Spec: specs/sim/units.md §4.2
//! Mode schedules (`units.md` §4.2) of a tick recording (`tick-raw-1`,
//! `record_tick.py` 0.2.0, which adds the `anim` record) replayed through
//! `d2_sim::units::anim::schedule`.
//!
//! Every `anim` record holds the inputs of one mode schedule as the
//! original read them at the scheduler: form (`fn` and its argument `b`
//! or `arg`), frame `f`, speed `sp`, frame count `fc`, current frame
//! `cur` and the AnimData event bytes (`ev`, non-zero bytes only; byte
//! −1 is the speed's high byte, `ad_speed >> 24`). The records that follow
//! it are what the original did: `cancel`s of the unit's type-0 / type-1
//! timers (the variants), then the `set`s of the schedule, ending with
//! the one ENDANIM (type 1). The harness computes the schedule with
//! d2-sim and requires, exactly (METHODS M01):
//!
//! * the recorded sets to be d2-sim's events in order: type, requested
//!   expire, a1, a2 (`check_units.py` U4 "anim exact");
//! * a cancel of every live type-0 / type-1 timer of the unit when d2-sim
//!   says the form cancels (`Schedule::cancels`), and none otherwise.
//!
//! Not compared: sequence animations (`seq: true`; the recorder does not
//! log the sequence's event bytes) and variants that read an event byte
//! outside the record (`AnimError::EventIndex`, `units.md` edge case 1);
//! both are counted. The recording's tables are not needed.

use std::collections::BTreeMap;

use d2_sim::units::anim::{schedule, AnimError, Events, Form, Scheduled};
use d2_sim::units::record::ANIM_EVENTS;
use serde_json::Value;

use crate::raw::{bad, Fields, HarnessError, Mismatch, RawRecording, TICK_RAW};

/// A unit as the recording names it: (unit type, GUID).
type Unit = (i64, i64);

/// What a replay compared.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct UnitStats {
    /// `anim` records read.
    pub anim_records: usize,
    /// Schedules compared exactly.
    pub schedules: usize,
    /// Timer sets compared exactly (ENDANIM included).
    pub events: usize,
    /// Cancels compared (variants).
    pub cancels: usize,
    /// Sequence animations, not computable from the record.
    pub sequences_skipped: usize,
    /// Variants reading an event byte outside the record.
    pub outside_record_skipped: usize,
}

/// One recorded `set` of a mode schedule.
#[derive(Clone, Copy, Debug)]
struct Set {
    at: usize,
    event: i64,
    req: i64,
    a1: i64,
    a2: i64,
}

fn unit_of(r: &Value) -> Option<Unit> {
    Some((r["ut"].as_i64()?, r["g"].as_i64()?))
}

fn timer_key(r: &Value) -> String {
    r["tm"].to_string()
}

/// The form and inputs of an `anim` record.
fn form_of(f: &Fields<'_>) -> Result<Form, HarnessError> {
    let func = crate::raw::parse_hex(f.str("fn")?)
        .ok_or_else(|| bad(f.name, f.i, "field \"fn\" is not hex"))?;
    Ok(match func {
        0x5539B0 => Form::Main { bonus: f.i32("b")? },
        0x553B10 => Form::Percent(f.i32("arg")?),
        0x553C70 => Form::Frames(f.i32("arg")?),
        0x553DC0 => Form::StartFrame(f.i32("arg")?),
        other => return Err(bad(f.name, f.i, format!("unknown scheduler {other:#x}"))),
    })
}

/// The event bytes and byte −1 of an `anim` record.
fn events_of(f: &Fields<'_>) -> Result<(u8, [u8; ANIM_EVENTS]), HarnessError> {
    let mut events = [0u8; ANIM_EVENTS];
    if let Some(list) = f.r["ev"].as_array() {
        for pair in list {
            let (Some(i), Some(v)) = (pair[0].as_u64(), pair[1].as_u64()) else {
                return Err(bad(f.name, f.i, "\"ev\" entry is not [index, value]"));
            };
            let slot = usize::try_from(i)
                .ok()
                .and_then(|i| events.get_mut(i))
                .ok_or_else(|| bad(f.name, f.i, format!("event index {i} ≥ {ANIM_EVENTS}")))?;
            *slot = u8::try_from(v).map_err(|_| bad(f.name, f.i, "event byte > 255"))?;
        }
    }
    // Byte −1 (+0x0F) is the speed's high byte (`check_units.py`).
    let speed = f.r["ad_speed"].as_i64().unwrap_or(0);
    Ok((((speed >> 24) & 0xFF) as u8, events))
}

/// Replays every `anim` record of a `tick-raw-1` recording.
pub fn replay_anim(rec: &RawRecording) -> Result<UnitStats, HarnessError> {
    if rec.format != TICK_RAW {
        return Err(bad(&rec.name, 0, format!("not a {TICK_RAW} recording")));
    }
    let name = rec.name.as_str();
    let records = &rec.records;
    let mismatch = |at: usize, field: &str, detail: String| {
        HarnessError::Mismatch(Mismatch {
            source: name.to_owned(),
            at: at as u64,
            field: field.to_owned(),
            detail,
        })
    };
    let mut stats = UnitStats::default();
    // Live timers: tm → (unit, type).
    let mut live: BTreeMap<String, (Unit, i64)> = BTreeMap::new();
    for (i, r) in records.iter().enumerate() {
        match r["k"].as_str() {
            Some("set") => {
                if let (Some(u), Some(ty)) = (unit_of(r), r["ty"].as_i64()) {
                    live.insert(timer_key(r), (u, ty));
                }
            }
            Some("cancel") => {
                live.remove(&timer_key(r));
            }
            Some("ex") if r["l"] == "d" => {
                live.remove(&timer_key(r));
            }
            Some("anim") => {
                stats.anim_records += 1;
                let f = Fields { name, i, r };
                if f.bool("seq")? {
                    stats.sequences_skipped += 1;
                    continue;
                }
                let unit = unit_of(r).ok_or_else(|| bad(name, i, "anim without a unit"))?;
                let form = form_of(&f)?;
                let (byte_0f, events) = events_of(&f)?;
                let computed = schedule(
                    form,
                    f.i32("f")?,
                    f.i32("sp")?,
                    f.i32("fc")?,
                    f.i32("cur")?,
                    Events::Record {
                        byte_0f,
                        events: &events,
                    },
                );
                let sched = match computed {
                    Ok(s) => s,
                    Err(AnimError::EventIndex(_)) => {
                        stats.outside_record_skipped += 1;
                        continue;
                    }
                    Err(e) => return Err(mismatch(i, "schedule", format!("d2-sim: {e}"))),
                };
                let (cancels, sets) = window(records, i, unit);
                // Cancels: d2-sim's variants cancel every live type-0 and
                // type-1 timer of the unit; the main form cancels none.
                let mode_timers: Vec<&String> = live
                    .iter()
                    .filter(|(_, (u, ty))| *u == unit && matches!(ty, 0 | 1))
                    .map(|(k, _)| k)
                    .collect();
                let d2_cancels = sched.as_ref().is_some_and(|s| s.cancels);
                for &(at, ref tm) in &cancels {
                    if mode_timers.contains(&tm) {
                        if !d2_cancels {
                            return Err(mismatch(
                                at,
                                "cancel",
                                format!(
                                    "timer {tm} cancelled; d2-sim's form {form:?} cancels none"
                                ),
                            ));
                        }
                        stats.cancels += 1;
                    }
                }
                if d2_cancels {
                    if let Some(tm) = mode_timers
                        .iter()
                        .find(|tm| !cancels.iter().any(|(_, c)| c == **tm))
                    {
                        return Err(mismatch(
                            i,
                            "cancel",
                            format!("d2-sim's form {form:?} cancels live timer {tm}; not recorded"),
                        ));
                    }
                }
                let want: Vec<Scheduled> = sched.map(|s| s.events).unwrap_or_default();
                compare(&sets, &want, i)
                    .map_err(|(at, field, detail)| mismatch(at, field, detail))?;
                stats.schedules += 1;
                stats.events += want.len();
            }
            _ => {}
        }
    }
    Ok(stats)
}

/// The records after the `anim` at `i`: cancels (index, timer) up to the
/// first set, then the unit's type-0 / type-1 sets of the delayed list up
/// to and including the ENDANIM.
fn window(records: &[Value], i: usize, unit: Unit) -> (Vec<(usize, String)>, Vec<Set>) {
    let mut cancels = Vec::new();
    let mut sets = Vec::new();
    for (j, r) in records.iter().enumerate().skip(i + 1) {
        match r["k"].as_str() {
            Some("cancel") if sets.is_empty() => cancels.push((j, timer_key(r))),
            Some("set")
                if unit_of(r) == Some(unit)
                    && r["l"] == "d"
                    && matches!(r["ty"].as_i64(), Some(0 | 1)) =>
            {
                let event = r["ty"].as_i64().unwrap_or(-1);
                let req = r["req"]
                    .as_i64()
                    .or_else(|| r["x"].as_i64())
                    .unwrap_or(i64::MIN);
                sets.push(Set {
                    at: j,
                    event,
                    req,
                    a1: r["a1"].as_i64().unwrap_or(-1),
                    a2: r["a2"].as_i64().unwrap_or(-1),
                });
                if event == 1 {
                    break;
                }
            }
            _ => break,
        }
    }
    (cancels, sets)
}

/// The first difference between the recorded sets and d2-sim's events:
/// (record, field, detail).
fn compare(
    got: &[Set],
    want: &[Scheduled],
    anim: usize,
) -> Result<(), (usize, &'static str, String)> {
    for (k, w) in want.iter().enumerate() {
        let Some(g) = got.get(k) else {
            // At the record where the missing set should be.
            let at = got.last().map_or(anim + 1, |g| g.at + 1);
            return Err((
                at,
                "group",
                format!(
                    "recorded schedule ends after {} sets; d2-sim schedules type {} at {} (a1 {}, a2 {}) next",
                    got.len(),
                    w.event,
                    w.expire,
                    w.a1,
                    w.a2
                ),
            ));
        };
        let fields: [(&'static str, i64, i64); 4] = [
            ("type", g.event, i64::from(w.event)),
            ("expire", g.req, i64::from(w.expire)),
            ("a1", g.a1, i64::from(w.a1)),
            ("a2", g.a2, i64::from(w.a2)),
        ];
        for (field, recorded, ours) in fields {
            if recorded != ours {
                return Err((
                    g.at,
                    field,
                    format!("recorded {recorded}, d2-sim {ours} (set {k} of the schedule)"),
                ));
            }
        }
    }
    if let Some(g) = got.get(want.len()) {
        return Err((
            g.at,
            "group",
            format!(
                "recorded set of type {} at {} beyond d2-sim's {} events",
                g.event,
                g.req,
                want.len()
            ),
        ));
    }
    Ok(())
}
