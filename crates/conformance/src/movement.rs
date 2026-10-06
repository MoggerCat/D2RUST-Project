// Spec: specs/sim/pathing.md §1.1, §9, §10; specs/sim/intents-events.md §2.4 rules 3–4
//! Walk / run replay: the movement requests of a recording (C→S 0x01–0x04)
//! go into a [`Mover`] in recorded order with the recorded ticks between
//! them, and the movement messages the mover queues (S→C 0x0D, 0x0F, 0x10,
//! 0x15, 0x96) and the positions they carry are compared with the
//! recording, exactly (METHODS M01).
//!
//! Input: a packets recording (`packets-raw-1`, `record_packets.py`; the
//! records `check_packets.py` reads). No recording format holds per-tick
//! path state yet (`pathing.md` open question 1), so the per-tick
//! positions compared are the ones the messages carry, in sub-tiles:
//! 0x15 and 0x0F / 0x10 (unit type, GUID, x, y) and 0x96 (x, y of the
//! client's own player). 0x0D's x, y is a walk target (`path-placement.md`
//! §12.2 rule 6), not a position: compared as message bytes only.
//! Layouts: `sim/server-messages.tsv`, `client-messages.tsv`, decoded with
//! `d2-proto`'s generated tables.
//!
//! Records read (`tools/trace-recorder/README.md`, record_packets):
//!
//! * `c2s` with id 0x01–0x04 → [`Mover::request`]; other ids are counted
//!   ([`MoveReadStats::other_inputs`]), not replayed: a recording whose
//!   other inputs move the player (waypoint, warp, skills) parts at the
//!   first message they cause, by design;
//! * `tick` → [`Mover::tick`];
//! * `s2c` with a movement id → compared; the first 0x15 of each unit is
//!   its game-entry placement (`path-placement.md` §11): it seeds the
//!   mover ([`Mover::seed`]) and is not compared (the placement harness,
//!   [`crate::placement`], compares it). Other `s2c` ids are ignored.
//!
//! Windows as in [`crate::packets`]: the recorded movement messages
//! between two inputs (a request or a tick) are the window of the earlier
//! input; at every input and at the end the mover's queued movement
//! messages must equal the window (client, bytes, order), and each
//! position a window message carries must equal the mover's position of
//! that unit then (precise 16.16 position >> 16). A differing message
//! names its field from the layout (`WalkVerify.x`); a message the mover
//! queued beyond the window is reported at the next input's `seq`
//! (`u64::MAX` at the end of the recording).
//!
//! The seam ([`Mover`]) is filled by the d2-sim path wiring
//! (`d2_sim::path::walk`); until then [`NotWired`] reports so, and CI
//! runs the harness on synthetic recordings with a test mover.

use std::collections::BTreeMap;

use d2_proto::client::{Run, RunToUnit, Walk, WalkToUnit};
use d2_proto::schema::{packed_get, FieldType, SizeRule};
use d2_proto::transport::server_message;
use d2_proto::FixedMessage;
use d2_server::seams::ClientId;
use serde_json::Value;

use crate::raw::{bad, encode_hex, Fields, HarnessError, Mismatch, RawRecording, PACKETS_RAW};

/// S→C ids compared by this harness (`pathing.md` §10 rule 6).
pub const MOVEMENT_IDS: [u8; 5] = [0x0D, 0x0F, 0x10, 0x15, 0x96];

/// A C→S movement request (`client-messages.tsv` 0x01–0x04).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MoveRequest {
    /// 0x01 walk to a point.
    Walk { x: u16, y: u16 },
    /// 0x02 walk to a unit.
    WalkToUnit { unit_type: u32, guid: u32 },
    /// 0x03 run to a point.
    Run { x: u16, y: u16 },
    /// 0x04 run to a unit.
    RunToUnit { unit_type: u32, guid: u32 },
}

impl MoveRequest {
    /// Decodes a C→S message; `None` for ids other than 0x01–0x04.
    pub fn decode(bytes: &[u8]) -> Option<Result<Self, String>> {
        let r = match *bytes.first()? {
            0x01 => Walk::decode(bytes).map(|m| Self::Walk { x: m.x, y: m.y }),
            0x02 => WalkToUnit::decode(bytes).map(|m| Self::WalkToUnit {
                unit_type: m.type_,
                guid: m.id,
            }),
            0x03 => Run::decode(bytes).map(|m| Self::Run { x: m.x, y: m.y }),
            0x04 => RunToUnit::decode(bytes).map(|m| Self::RunToUnit {
                unit_type: m.type_,
                guid: m.id,
            }),
            _ => return None,
        };
        Some(r.map_err(|e| e.to_string()))
    }

    /// Running (mode 3) rather than walking (mode 2), `pathing.md` §1.1.
    pub fn runs(&self) -> bool {
        matches!(self, Self::Run { .. } | Self::RunToUnit { .. })
    }
}

/// A precise position: 16.16 sub-tiles (`path-placement.md` §1 rule 2).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Precise {
    pub x: u32,
    pub y: u32,
}

impl Precise {
    /// The sub-tile cell centre (fraction 0x8000).
    pub fn centre(x: u16, y: u16) -> Self {
        Self {
            x: (u32::from(x) << 16) | 0x8000,
            y: (u32::from(y) << 16) | 0x8000,
        }
    }

    /// Sub-tile coordinates (the messages' x, y).
    pub fn sub_tile(&self) -> (u16, u16) {
        ((self.x >> 16) as u16, (self.y >> 16) as u16)
    }
}

/// One event of a movement recording, in recorded order. `at` is the
/// record's `seq` (as `check_packets.py` numbers it).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MoveEvent {
    /// A unit's first 0x15 (game entry): the mover starts it there.
    Seed {
        at: u64,
        client: ClientId,
        unit_type: u8,
        guid: u32,
        x: u16,
        y: u16,
    },
    Request {
        at: u64,
        frame: u32,
        client: ClientId,
        request: MoveRequest,
    },
    Tick {
        at: u64,
        frame: u32,
    },
    /// A recorded movement message (id in [`MOVEMENT_IDS`]).
    Sent {
        at: u64,
        frame: u32,
        client: ClientId,
        bytes: Vec<u8>,
    },
}

/// What the reader kept and skipped.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MoveReadStats {
    pub requests: usize,
    pub ticks: usize,
    pub seeds: usize,
    /// Movement messages by id.
    pub sent: BTreeMap<u8, usize>,
    /// C→S game messages other than 0x01–0x04 (not replayed).
    pub other_inputs: usize,
}

/// A packets recording reduced to its movement events.
#[derive(Clone, Debug)]
pub struct MovementRecording {
    pub name: String,
    pub events: Vec<MoveEvent>,
    pub read: MoveReadStats,
}

/// Reads the movement events of a `packets-raw-1` recording.
pub fn read_movement(rec: &RawRecording) -> Result<MovementRecording, HarnessError> {
    if rec.format != PACKETS_RAW {
        return Err(bad(&rec.name, 0, format!("not a {PACKETS_RAW} recording")));
    }
    let name = rec.name.as_str();
    let mut events = Vec::new();
    let mut read = MoveReadStats::default();
    let mut seeded: Vec<(u8, u32)> = Vec::new();
    let mut frame = 0u32;
    for (i, r) in rec.records.iter().enumerate() {
        let f = Fields { name, i, r };
        let kind = r["type"].as_str().unwrap_or("");
        if !matches!(kind, "c2s" | "tick" | "s2c") {
            continue;
        }
        let at = r["seq"]
            .as_u64()
            .ok_or_else(|| bad(name, i, "field \"seq\" is not an integer"))?;
        if let Some(fr) = r["frame"].as_u64() {
            frame = u32::try_from(fr).map_err(|_| bad(name, i, "frame out of range"))?;
        }
        match kind {
            "tick" => {
                read.ticks += 1;
                events.push(MoveEvent::Tick { at, frame });
            }
            "c2s" => {
                let bytes = f.bytes("bytes")?;
                match MoveRequest::decode(&bytes) {
                    None => read.other_inputs += 1,
                    Some(Err(e)) => return Err(bad(name, i, e)),
                    Some(Ok(request)) => {
                        read.requests += 1;
                        events.push(MoveEvent::Request {
                            at,
                            frame,
                            client: f.u32("client")?,
                            request,
                        });
                    }
                }
            }
            _ => {
                let bytes = f.bytes("bytes")?;
                let Some(&id) = bytes.first() else { continue };
                if !MOVEMENT_IDS.contains(&id) {
                    continue;
                }
                let size = usize::try_from(f.u32("size")?).unwrap_or(usize::MAX);
                let fixed = match server_message(id).map(|m| m.size) {
                    Some(SizeRule::Fixed(n)) => usize::from(n),
                    _ => return Err(bad(name, i, format!("no fixed size for 0x{id:02X}"))),
                };
                if size != fixed || bytes.len() != fixed {
                    return Err(bad(
                        name,
                        i,
                        format!(
                            "0x{id:02X}: size {size}, {} bytes logged, layout {fixed}",
                            bytes.len()
                        ),
                    ));
                }
                let client = f.u32("client")?;
                if id == 0x15 {
                    let m = d2_proto::server::ReassignPlayer::decode(&bytes)
                        .map_err(|e| bad(name, i, e))?;
                    if !seeded.contains(&(m.type_, m.guid)) {
                        seeded.push((m.type_, m.guid));
                        read.seeds += 1;
                        events.push(MoveEvent::Seed {
                            at,
                            client,
                            unit_type: m.type_,
                            guid: m.guid,
                            x: m.x,
                            y: m.y,
                        });
                        continue;
                    }
                }
                *read.sent.entry(id).or_default() += 1;
                events.push(MoveEvent::Sent {
                    at,
                    frame,
                    client,
                    bytes,
                });
            }
        }
    }
    Ok(MovementRecording {
        name: rec.name.clone(),
        events,
        read,
    })
}

/// The system under test: the walk / run rules of `sim/pathing.md` behind
/// the game's message handlers and tick.
pub trait Mover {
    /// Puts a unit at its recorded game-entry cell (centre); for a player
    /// (`unit_type` 0) `client` is the client it belongs to.
    fn seed(
        &mut self,
        client: ClientId,
        unit_type: u8,
        guid: u32,
        x: u16,
        y: u16,
    ) -> Result<(), String>;
    /// A movement request of `client`'s player, as the dispatcher passes
    /// it (`pathing.md` §1.1).
    fn request(&mut self, client: ClientId, request: &MoveRequest) -> Result<(), String>;
    /// One game tick (`tick.md` §3).
    fn tick(&mut self) -> Result<(), String>;
    /// The S→C messages queued since the last call, in order (any ids:
    /// the harness keeps [`MOVEMENT_IDS`]).
    fn take_sent(&mut self) -> Vec<(ClientId, Vec<u8>)>;
    /// The unit's current position, `None` if the mover has no such unit.
    fn position(&self, unit_type: u8, guid: u32) -> Option<Precise>;
}

/// The seam before the d2-sim path wiring fills it: every call fails.
#[derive(Clone, Copy, Debug, Default)]
pub struct NotWired;

/// What [`NotWired`] reports.
pub const NOT_WIRED: &str =
    "MOVER NOT WIRED: no d2-sim mover behind conformance::movement::Mover yet \
     (docs/handoff/conformance-path-render.md)";

impl Mover for NotWired {
    fn seed(&mut self, _: ClientId, _: u8, _: u32, _: u16, _: u16) -> Result<(), String> {
        Err(NOT_WIRED.to_owned())
    }
    fn request(&mut self, _: ClientId, _: &MoveRequest) -> Result<(), String> {
        Err(NOT_WIRED.to_owned())
    }
    fn tick(&mut self) -> Result<(), String> {
        Err(NOT_WIRED.to_owned())
    }
    fn take_sent(&mut self) -> Vec<(ClientId, Vec<u8>)> {
        Vec::new()
    }
    fn position(&self, _: u8, _: u32) -> Option<Precise> {
        None
    }
}

/// What to compare.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MoveOptions {
    /// Movement message bytes.
    pub messages: bool,
    /// The positions the messages carry against [`Mover::position`].
    pub positions: bool,
}

impl Default for MoveOptions {
    fn default() -> Self {
        Self {
            messages: true,
            positions: true,
        }
    }
}

/// What a replay compared.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MoveStats {
    pub requests: usize,
    pub ticks: usize,
    pub seeds: usize,
    /// Movement messages compared byte for byte.
    pub messages: usize,
    /// Positions compared.
    pub positions: usize,
}

/// The unit and position a movement message carries, if it carries one
/// (0x0D's x, y is a target). 0x96 names no unit: `players` maps a client
/// to its player's GUID.
fn carried_position(
    client: ClientId,
    bytes: &[u8],
    players: &BTreeMap<ClientId, u32>,
) -> Option<Result<(u8, u32, u16, u16), String>> {
    use d2_proto::server::{PlayerMove, PlayerToTarget, ReassignPlayer, WalkVerify};
    let r = match bytes.first()? {
        0x0F => PlayerMove::decode(bytes).map(|m| Some((m.type_, m.guid, m.x, m.y))),
        0x10 => PlayerToTarget::decode(bytes).map(|m| Some((m.type_, m.guid, m.x, m.y))),
        0x15 => ReassignPlayer::decode(bytes).map(|m| Some((m.type_, m.guid, m.x, m.y))),
        0x96 => WalkVerify::decode(bytes).map(|m| players.get(&client).map(|&g| (0, g, m.x, m.y))),
        _ => return None,
    };
    match r {
        Ok(Some(p)) => Some(Ok(p)),
        Ok(None) => Some(Err(format!(
            "0x96 to client {client}: no player of that client seeded (no game-entry 0x15)"
        ))),
        Err(e) => Some(Err(e.to_string())),
    }
}

/// The first field of the layout where two messages of the same id
/// differ (`Name.field`), else the first differing byte (`bytes[k]`).
pub fn first_field_difference(recorded: &[u8], ours: &[u8]) -> Option<String> {
    if recorded == ours {
        return None;
    }
    if let (Some(&id), true) = (recorded.first(), recorded.len() == ours.len()) {
        if ours[0] == id {
            if let Some(m) = server_message(id) {
                for fd in m.layout {
                    let get = |b: &[u8]| -> Option<u32> {
                        let at = |o: u16, n: usize| b.get(o as usize..o as usize + n);
                        match (fd.ty, fd.offset) {
                            (FieldType::U8, Some(o)) => at(o, 1).map(|s| u32::from(s[0])),
                            (FieldType::U16, Some(o)) => {
                                at(o, 2).map(|s| u32::from(u16::from_le_bytes([s[0], s[1]])))
                            }
                            (FieldType::U32, Some(o)) => {
                                at(o, 4).map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
                            }
                            (FieldType::Packed { bit, width }, None) => {
                                Some(packed_get(b, bit as usize, u32::from(width)))
                            }
                            _ => None,
                        }
                    };
                    if let (Some(a), Some(b)) = (get(recorded), get(ours)) {
                        if a != b {
                            return Some(format!("{}.{}", m.name, fd.name));
                        }
                    }
                }
            }
        }
    }
    let k = (0..recorded.len().max(ours.len()))
        .find(|&k| recorded.get(k) != ours.get(k))
        .unwrap_or(0);
    Some(format!("bytes[{k}]"))
}

/// Replays a movement recording through `mover`.
pub fn replay_movement(
    rec: &MovementRecording,
    mover: &mut dyn Mover,
    opts: MoveOptions,
) -> Result<MoveStats, HarnessError> {
    let name = rec.name.as_str();
    let mismatch = |at: u64, field: &str, detail: String| {
        HarnessError::Mismatch(Mismatch {
            source: name.to_owned(),
            at,
            field: field.to_owned(),
            detail,
        })
    };
    let fail = |at: u64, what: &str, e: String| {
        HarnessError::Trace(crate::TraceError::Format(format!(
            "{name}: record seq {at}: mover {what}: {e}"
        )))
    };
    let mut stats = MoveStats::default();
    let mut players: BTreeMap<ClientId, u32> = BTreeMap::new();
    // (seq, client, bytes) of the recorded window.
    let mut window: Vec<(u64, ClientId, &[u8])> = Vec::new();

    let close = |mover: &mut dyn Mover,
                 window: &mut Vec<(u64, ClientId, &[u8])>,
                 players: &BTreeMap<ClientId, u32>,
                 stats: &mut MoveStats,
                 next: u64|
     -> Result<(), HarnessError> {
        let ours: Vec<(ClientId, Vec<u8>)> = mover
            .take_sent()
            .into_iter()
            .filter(|(_, b)| b.first().is_some_and(|id| MOVEMENT_IDS.contains(id)))
            .collect();
        if opts.messages {
            for (k, &(at, client, bytes)) in window.iter().enumerate() {
                let Some((c, b)) = ours.get(k) else {
                    return Err(mismatch(
                        at,
                        "s2c",
                        format!(
                            "recorded {} to client {client} not queued by the mover",
                            encode_hex(bytes)
                        ),
                    ));
                };
                if *c != client {
                    return Err(mismatch(
                        at,
                        "client",
                        format!("recorded client {client}, mover {c}"),
                    ));
                }
                if let Some(field) = first_field_difference(bytes, b) {
                    return Err(mismatch(
                        at,
                        &field,
                        format!("recorded {}, mover {}", encode_hex(bytes), encode_hex(b)),
                    ));
                }
                stats.messages += 1;
            }
            if let Some((c, b)) = ours.get(window.len()) {
                return Err(mismatch(
                    next,
                    "s2c",
                    format!(
                        "mover queued {} to client {c} beyond the {} recorded messages before this record",
                        encode_hex(b),
                        window.len()
                    ),
                ));
            }
        }
        if opts.positions {
            for &(at, client, bytes) in window.iter() {
                let Some(p) = carried_position(client, bytes, players) else {
                    continue;
                };
                let (ut, guid, x, y) = p.map_err(|e| bad(name, 0, format!("seq {at}: {e}")))?;
                let Some(ours) = mover.position(ut, guid) else {
                    return Err(mismatch(
                        at,
                        "position",
                        format!("unit {ut}:{guid} at ({x}, {y}) unknown to the mover"),
                    ));
                };
                let (ox, oy) = ours.sub_tile();
                if ox != x || oy != y {
                    let field = if ox != x { "position.x" } else { "position.y" };
                    return Err(mismatch(
                        at,
                        field,
                        format!(
                            "unit {ut}:{guid}: recorded ({x}, {y}), mover ({ox}, {oy}) = precise (0x{:X}, 0x{:X})",
                            ours.x, ours.y
                        ),
                    ));
                }
                stats.positions += 1;
            }
        }
        window.clear();
        Ok(())
    };

    for ev in &rec.events {
        match ev {
            MoveEvent::Seed {
                at,
                client,
                unit_type,
                guid,
                x,
                y,
            } => {
                if *unit_type == 0 {
                    players.entry(*client).or_insert(*guid);
                }
                mover
                    .seed(*client, *unit_type, *guid, *x, *y)
                    .map_err(|e| fail(*at, "seed", e))?;
                stats.seeds += 1;
            }
            MoveEvent::Sent {
                at, client, bytes, ..
            } => window.push((*at, *client, bytes)),
            MoveEvent::Request {
                at,
                client,
                request,
                ..
            } => {
                close(mover, &mut window, &players, &mut stats, *at)?;
                mover
                    .request(*client, request)
                    .map_err(|e| fail(*at, "request", e))?;
                stats.requests += 1;
            }
            MoveEvent::Tick { at, .. } => {
                close(mover, &mut window, &players, &mut stats, *at)?;
                mover.tick().map_err(|e| fail(*at, "tick", e))?;
                stats.ticks += 1;
            }
        }
    }
    close(mover, &mut window, &players, &mut stats, u64::MAX)?;
    Ok(stats)
}

/// Reads and replays one recording file's records.
pub fn replay_recording(
    rec: &RawRecording,
    mover: &mut dyn Mover,
    opts: MoveOptions,
) -> Result<MoveStats, HarnessError> {
    replay_movement(&read_movement(rec)?, mover, opts)
}

/// Builds a `packets-raw-1` `s2c` / `c2s` record (synthetic recordings).
pub fn packet_record(kind: &str, seq: u64, frame: u32, client: ClientId, bytes: &[u8]) -> Value {
    serde_json::json!({
        "type": kind,
        "seq": seq,
        "frame": frame,
        "client": client,
        "size": bytes.len(),
        "bytes": encode_hex(bytes),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use d2_proto::server::WalkVerify;

    #[test]
    fn requests_decode_from_the_client_layouts() {
        assert_eq!(
            MoveRequest::decode(&[0x01, 0x69, 0x00, 0x64, 0x00]),
            Some(Ok(MoveRequest::Walk { x: 105, y: 100 }))
        );
        assert_eq!(
            MoveRequest::decode(&[0x04, 1, 0, 0, 0, 7, 0, 0, 0]),
            Some(Ok(MoveRequest::RunToUnit {
                unit_type: 1,
                guid: 7
            }))
        );
        assert!(MoveRequest::decode(&[0x01, 0x69, 0x00]).unwrap().is_err());
        assert_eq!(MoveRequest::decode(&[0x13, 0, 0, 0, 0]), None);
    }

    #[test]
    fn field_difference_names_the_packed_field() {
        let a = WalkVerify {
            stamina: 100,
            x: 101,
            y: 100,
            dx: 0,
            dy: 0,
        }
        .encode();
        let mut b = WalkVerify {
            x: 102,
            ..WalkVerify::decode(&a).unwrap()
        }
        .encode();
        assert_eq!(
            first_field_difference(&a, &b).as_deref(),
            Some("WalkVerify.x")
        );
        b = a;
        assert_eq!(first_field_difference(&a, &b), None);
        assert_eq!(
            first_field_difference(&[0x96, 2], &[0x96, 2, 0]).as_deref(),
            Some("bytes[2]")
        );
    }

    #[test]
    fn precise_cell_centre() {
        let p = Precise::centre(100, 5);
        assert_eq!((p.x, p.y), (0x648000, 0x58000));
        assert_eq!(p.sub_tile(), (100, 5));
    }
}
