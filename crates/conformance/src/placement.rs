// Spec: specs/sim/path-placement.md §7, §9–§12; specs/monsters/population.md; specs/items/treasure.md §7
//! Spawn placement replay: the recorded positions of the units placed in
//! each room (players, monsters, items) against where d2rs places them,
//! through a [`PlacementModel`], exactly (METHODS M01).
//!
//! The core ([`replay_placement`]) takes [`Spawn`] records, groups them by
//! (room, kind) in order of first appearance and asks the model for the
//! same group: same count, and per spawn in order the same GUID, class
//! (when recorded) and sub-tile position. The first difference names the
//! recorded spawn (`at` = its record's `seq`); a spawn d2rs makes beyond
//! the recorded ones is reported at the group's last recorded spawn.
//!
//! What recordings carry today (`packets-raw-1`, read by
//! [`read_player_placements`]): player placements only, from S→C 0x15
//! (type, GUID, x, y; `path-placement.md` §10 rule 6, §11; vectors
//! R1–R3) with the room from the S→C 0x07 MapReveal sent since the
//! previous 0x15 when there is exactly one (warp / waypoint arrival:
//! tile x, tile y and level of the destination room); several (game
//! entry sends one per room near the spawn, R2) leave the room unnamed.
//! Monster positions (S→C 0xAC) and item positions (0x9C) are in those
//! recordings but their byte positions are not in any spec
//! (`server-messages.tsv` gives no 0xAC layout; 0x9C's `data@8` is not
//! broken down): the reader counts them and does not decode them. A
//! monster / item reader comes with the recorder extension named in
//! `path-placement.md` open question 1 (or with those layouts);
//! `docs/handoff/conformance-path-render.md` lists what it must record.

use d2_proto::server::{MapReveal, ReassignPlayer};
use d2_proto::FixedMessage;

use crate::raw::{bad, Fields, HarnessError, Mismatch, RawRecording, PACKETS_RAW};

/// A room as the protocol names it: the 0x07 MapReveal fields (tile
/// coordinates of the room and its level id).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RoomKey {
    pub level: u8,
    pub tile_x: u16,
    pub tile_y: u16,
}

/// Which placement rule put the unit there.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SpawnKind {
    /// `path-placement.md` §10–§12 (game entry, warps, waypoints).
    Player,
    /// `monsters/population.md` (room population, presets, packs).
    Monster,
    /// `path-placement.md` §9, `items/treasure.md` §7 (floor drops).
    Item,
}

/// One recorded placement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Spawn {
    /// The record (`seq`) that shows it.
    pub at: u64,
    pub frame: u32,
    /// `None`: the recording does not say which room.
    pub room: Option<RoomKey>,
    pub kind: SpawnKind,
    pub unit_type: u8,
    pub guid: u32,
    /// Class id when the recording has it.
    pub class: Option<u32>,
    pub x: u16,
    pub y: u16,
}

/// One unit as d2rs places it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Placed {
    pub guid: u32,
    pub class: u32,
    pub x: u16,
    pub y: u16,
}

/// The system under test.
pub trait PlacementModel {
    /// The units of `kind` d2rs places in `room` (`None`: the placements
    /// whose room the recording does not name, e.g. game entry), in
    /// placement order. `frame` is the first recorded spawn's frame.
    fn placed(
        &mut self,
        room: Option<RoomKey>,
        kind: SpawnKind,
        frame: u32,
    ) -> Result<Vec<Placed>, String>;
}

/// The seam before the d2-sim wiring fills it.
#[derive(Clone, Copy, Debug, Default)]
pub struct NotWired;

/// What [`NotWired`] reports.
pub const NOT_WIRED: &str =
    "PLACEMENT NOT WIRED: no d2-sim placement behind conformance::placement::PlacementModel yet \
     (docs/handoff/conformance-path-render.md)";

impl PlacementModel for NotWired {
    fn placed(&mut self, _: Option<RoomKey>, _: SpawnKind, _: u32) -> Result<Vec<Placed>, String> {
        Err(NOT_WIRED.to_owned())
    }
}

/// What the packets reader found.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PlacementReadStats {
    pub players: usize,
    /// Player placements with a named room.
    pub with_room: usize,
    /// 0xAC monster assigns seen, not decoded (no layout in the specs).
    pub monster_assigns: usize,
    /// 0x9C world item messages seen, not decoded.
    pub item_messages: usize,
}

/// The player placements of a `packets-raw-1` recording.
pub fn read_player_placements(
    rec: &RawRecording,
) -> Result<(Vec<Spawn>, PlacementReadStats), HarnessError> {
    if rec.format != PACKETS_RAW {
        return Err(bad(&rec.name, 0, format!("not a {PACKETS_RAW} recording")));
    }
    let name = rec.name.as_str();
    let mut out = Vec::new();
    let mut stats = PlacementReadStats::default();
    let mut reveals: Vec<RoomKey> = Vec::new();
    let mut frame = 0u32;
    for (i, r) in rec.records.iter().enumerate() {
        if let Some(fr) = r["frame"].as_u64() {
            frame = u32::try_from(fr).map_err(|_| bad(name, i, "frame out of range"))?;
        }
        if r["type"] != "s2c" {
            continue;
        }
        let f = Fields { name, i, r };
        let bytes = f.bytes("bytes")?;
        match bytes.first() {
            Some(0x07) => {
                let m = MapReveal::decode(&bytes).map_err(|e| bad(name, i, e))?;
                reveals.push(RoomKey {
                    level: m.level,
                    tile_x: m.x,
                    tile_y: m.y,
                });
            }
            Some(0x15) => {
                let m = ReassignPlayer::decode(&bytes).map_err(|e| bad(name, i, e))?;
                let room = (reveals.len() == 1).then(|| reveals[0]);
                stats.players += 1;
                stats.with_room += usize::from(room.is_some());
                out.push(Spawn {
                    at: r["seq"]
                        .as_u64()
                        .ok_or_else(|| bad(name, i, "field \"seq\" is not an integer"))?,
                    frame,
                    room,
                    kind: SpawnKind::Player,
                    unit_type: m.type_,
                    guid: m.guid,
                    class: None,
                    x: m.x,
                    y: m.y,
                });
                reveals.clear();
            }
            Some(0xAC) => stats.monster_assigns += 1,
            Some(0x9C) => stats.item_messages += 1,
            _ => {}
        }
    }
    Ok((out, stats))
}

/// A (room, kind) group.
type GroupKey = (Option<RoomKey>, SpawnKind);

/// What a replay compared.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PlacementStats {
    /// (room, kind) groups compared.
    pub groups: usize,
    pub spawns: usize,
}

/// Compares `spawns` (recorded order) with `model`, group by group.
pub fn replay_placement(
    source: &str,
    spawns: &[Spawn],
    model: &mut dyn PlacementModel,
) -> Result<PlacementStats, HarnessError> {
    let mismatch = |at: u64, field: &str, detail: String| {
        HarnessError::Mismatch(Mismatch {
            source: source.to_owned(),
            at,
            field: field.to_owned(),
            detail,
        })
    };
    // Groups in order of first appearance (no hash order, M01-stable).
    let mut groups: Vec<(GroupKey, Vec<&Spawn>)> = Vec::new();
    for s in spawns {
        let key = (s.room, s.kind);
        match groups.iter_mut().find(|(k, _)| *k == key) {
            Some((_, v)) => v.push(s),
            None => groups.push((key, vec![s])),
        }
    }
    let mut stats = PlacementStats::default();
    for ((room, kind), recorded) in &groups {
        let shown = match room {
            Some(r) => format!("room level {} tile ({}, {})", r.level, r.tile_x, r.tile_y),
            None => "unnamed room".to_owned(),
        };
        let ours = model.placed(*room, *kind, recorded[0].frame).map_err(|e| {
            HarnessError::Trace(crate::TraceError::Format(format!(
                "{source}: record seq {}: placement model: {e}",
                recorded[0].at
            )))
        })?;
        for (k, s) in recorded.iter().enumerate() {
            let Some(p) = ours.get(k) else {
                return Err(mismatch(
                    s.at,
                    "missing",
                    format!(
                        "{kind:?} {}:{} at ({}, {}) in {shown}: d2rs places only {}",
                        s.unit_type,
                        s.guid,
                        s.x,
                        s.y,
                        ours.len()
                    ),
                ));
            };
            let field = if p.guid != s.guid {
                Some("guid")
            } else if s.class.is_some_and(|c| c != p.class) {
                Some("class")
            } else if p.x != s.x {
                Some("x")
            } else if p.y != s.y {
                Some("y")
            } else {
                None
            };
            if let Some(field) = field {
                return Err(mismatch(
                    s.at,
                    field,
                    format!(
                        "{kind:?} #{k} in {shown}: recorded guid {} class {:?} ({}, {}), d2rs guid {} class {} ({}, {})",
                        s.guid, s.class, s.x, s.y, p.guid, p.class, p.x, p.y
                    ),
                ));
            }
            stats.spawns += 1;
        }
        if let Some(p) = ours.get(recorded.len()) {
            let last = recorded[recorded.len() - 1];
            return Err(mismatch(
                last.at,
                "extra",
                format!(
                    "{kind:?} in {shown}: d2rs places guid {} class {} at ({}, {}) beyond the {} recorded",
                    p.guid,
                    p.class,
                    p.x,
                    p.y,
                    recorded.len()
                ),
            ));
        }
        stats.groups += 1;
    }
    Ok(stats)
}
