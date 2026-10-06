// Spec: specs/drlg/rooms.md §6
//! Adjacency arrays (`rooms.md` §6) of a format-1 tick trace
//! (`traces/sim/tick/*.json`, written by `convert_tick.py`; the format
//! `check_rooms.py` reads) replayed through a [`RoomModel`] and compared
//! with every `lists` snapshot.
//!
//! The trace gives room activations and deactivations (`room_activate`,
//! `room_deactivate`), the player's room changes (`room_add` /
//! `room_remove` of unit type 0) and, per snapshot, every active room's
//! adjacency array (`acts[a][r].adj`), all merged by `data.seq`; a
//! snapshot is the state after every event with a smaller seq. The
//! harness drives the model with the events in that order and requires,
//! at each snapshot, the snapshot's rooms to be the replayed active set
//! and every array the model can tell to equal the recorded one entry by
//! entry (METHODS M01). The first difference is reported by the
//! snapshot's seq and the path `acts[a][r].adj[k]`.
//!
//! The model is the system under test. d2-sim's arrays live in
//! `d2_sim::drlg::Drlg` (fill §6.1, refill on activation §6.2,
//! swap-with-last removal §6.3) and are computed from the rooms-near
//! order (§3), i.e. from the DRLG rooms' tile rects, which the tick
//! recordings do not hold. A `Drlg`-backed model needs the recorder
//! extension of `rooms.md` Test vectors ("Full check": per active room
//! the DRLG room, its rect, level and rooms-near array); its format is
//! not defined yet. Until then the harness runs with the models of the
//! tests (`tests/rooms_replay.rs`), and [`RoomModel::adjacency`] may say
//! `None` for arrays the model cannot tell (counted, not compared).

use std::collections::BTreeSet;

use serde_json::Value;

use crate::raw::{HarnessError, Mismatch};
use crate::{Trace, TraceError};

/// The system under test: the active rooms and their adjacency arrays.
/// Rooms are named as the trace names them (`"R12"`).
pub trait RoomModel {
    /// `room_activate`: `room` became an active room of `act`.
    fn activate(&mut self, act: u8, room: &str);
    /// `room_deactivate`: `room` was removed (tick step 9, §8.2).
    fn deactivate(&mut self, act: u8, room: &str);
    /// The player unit `unit` entered `room` (`room_add`), or left its
    /// room (`room_remove`, `None`).
    fn player_room(&mut self, unit: (u8, u32), room: Option<&str>) {
        let _ = (unit, room);
    }
    /// The adjacency array of an active room, index 0 first; `None` when
    /// the model cannot tell.
    fn adjacency(&self, room: &str) -> Option<Vec<String>>;
}

/// What a replay compared.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RoomStats {
    pub activations: usize,
    pub deactivations: usize,
    pub player_moves: usize,
    pub snapshots: usize,
    /// Arrays compared entry by entry.
    pub arrays: usize,
    /// Arrays the model could not tell.
    pub arrays_unknown: usize,
}

enum Ev<'a> {
    Activate(u8, &'a str),
    Deactivate(u8, &'a str),
    Player((u8, u32), Option<&'a str>),
    Lists(&'a Value),
}

fn format(trace: &Trace, m: String) -> HarnessError {
    HarnessError::Trace(TraceError::Format(format!("trace {}: {m}", trace.id)))
}

/// The room events and snapshots of a trace, by seq.
fn events(trace: &Trace) -> Result<Vec<(u64, Ev<'_>)>, HarnessError> {
    let mut out = Vec::new();
    for e in trace.inputs.iter().chain(&trace.expected) {
        let d = &e["data"];
        let seq = d["seq"]
            .as_u64()
            .ok_or_else(|| format(trace, "event without data.seq".into()))?;
        let room = || {
            d["room"]
                .as_str()
                .ok_or_else(|| format(trace, format!("seq {seq}: no room")))
        };
        let act = || {
            d["act"]
                .as_u64()
                .and_then(|a| u8::try_from(a).ok())
                .ok_or_else(|| format(trace, format!("seq {seq}: no act")))
        };
        let unit = || -> Option<(u8, u32)> {
            Some((
                u8::try_from(d["unit"][0].as_u64()?).ok()?,
                u32::try_from(d["unit"][1].as_u64()?).ok()?,
            ))
        };
        let ev = match e["kind"].as_str() {
            Some("room_activate") => Ev::Activate(act()?, room()?),
            Some("room_deactivate") => Ev::Deactivate(act()?, room()?),
            Some("room_add") if d["unit"][0] == 0 => Ev::Player(
                unit().ok_or_else(|| format(trace, format!("seq {seq}: bad unit")))?,
                Some(room()?),
            ),
            Some("room_remove") if d["unit"][0] == 0 => Ev::Player(
                unit().ok_or_else(|| format(trace, format!("seq {seq}: bad unit")))?,
                None,
            ),
            Some("lists") => Ev::Lists(d),
            _ => continue,
        };
        out.push((seq, ev));
    }
    out.sort_by_key(|(seq, _)| *seq);
    Ok(out)
}

/// Replays a tick trace's room events through `model` and compares every
/// snapshot's adjacency arrays.
pub fn replay_rooms(trace: &Trace, model: &mut dyn RoomModel) -> Result<RoomStats, HarnessError> {
    if (trace.area.as_str(), trace.behavior.as_str()) != ("sim", "tick") {
        return Err(format(trace, "not a sim/tick trace".into()));
    }
    let mismatch = |at: u64, field: String, detail: String| {
        HarnessError::Mismatch(Mismatch {
            source: trace.id.clone(),
            at,
            field,
            detail,
        })
    };
    let mut stats = RoomStats::default();
    let mut active = BTreeSet::new();
    for (seq, ev) in events(trace)? {
        match ev {
            Ev::Activate(act, room) => {
                stats.activations += 1;
                active.insert(room.to_owned());
                model.activate(act, room);
            }
            Ev::Deactivate(act, room) => {
                stats.deactivations += 1;
                active.remove(room);
                model.deactivate(act, room);
            }
            Ev::Player(unit, room) => {
                stats.player_moves += 1;
                model.player_room(unit, room);
            }
            Ev::Lists(d) => {
                stats.snapshots += 1;
                let acts = d["acts"]
                    .as_array()
                    .ok_or_else(|| format(trace, format!("seq {seq}: snapshot without acts")))?;
                let mut listed = BTreeSet::new();
                for (a, rooms) in acts.iter().enumerate() {
                    for (r, room) in rooms.as_array().into_iter().flatten().enumerate() {
                        let name = room["room"]
                            .as_str()
                            .ok_or_else(|| format(trace, format!("seq {seq}: room name")))?;
                        listed.insert(name.to_owned());
                        let recorded: Vec<String> = room["adj"]
                            .as_array()
                            .ok_or_else(|| format(trace, format!("seq {seq}: {name} without adj")))?
                            .iter()
                            .map(|v| v.as_str().unwrap_or("?").to_owned())
                            .collect();
                        let Some(ours) = model.adjacency(name) else {
                            stats.arrays_unknown += 1;
                            continue;
                        };
                        stats.arrays += 1;
                        let path = |k: usize| format!("acts[{a}][{r}].adj[{k}]");
                        let n = recorded.len().max(ours.len());
                        if let Some(k) = (0..n).find(|&k| recorded.get(k) != ours.get(k)) {
                            return Err(mismatch(
                                seq,
                                path(k),
                                format!("room {name}: recorded {recorded:?}, model {ours:?}"),
                            ));
                        }
                    }
                }
                if listed != active {
                    let diff: Vec<_> = listed.symmetric_difference(&active).collect();
                    return Err(mismatch(
                        seq,
                        "rooms".into(),
                        format!("snapshot rooms differ from the replayed active set: {diff:?}"),
                    ));
                }
            }
        }
    }
    Ok(stats)
}
