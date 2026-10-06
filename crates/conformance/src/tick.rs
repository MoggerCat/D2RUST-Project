//! Replays `sim/tick` traces (`traces/sim/tick/*.json`, written by
//! `tools/trace-recorder/convert_tick.py`) through `d2_sim::tick::tick`,
//! the timer queue and `UnitLists` (`specs/sim/tick.md`,
//! `specs/sim/unit-order.md`, Test vectors).
//!
//! The trace holds every timer schedule and cancel and every list insert
//! and removal of a recorded game as `inputs`, and every timer run and
//! list snapshot as `expected`, ordered by one `seq` and labelled with the
//! tick step they happened in. The replay runs `tick()` once per recorded
//! tick with a [`TickHooks`] implementation that applies each step's
//! recorded inputs at that step's hooks (the step bodies are owned by
//! specs not written yet), and an [`EventDispatch`] that compares every
//! timer d2-sim runs with the next recorded run and then applies what the
//! recorded handler did. Comparison is exact (METHODS M01): every timer
//! run in order and every snapshot (the adjacent-room arrays aside, see
//! [`replay`]).
//!
//! The recording logs list primitives; d2-sim's list API is coarser
//! (unit-order.md §3: adding a unit places it in its room, queues it and
//! hashes it). Each recorded primitive picks the d2-sim operation it
//! starts; the primitives that operation performs next are owed, and the
//! recording must show exactly those next. Work d2-sim does by itself
//! (step 6 queue clears, step 9 room deactivation, freeing a timer after
//! its run) is never replayed from the recording: the recording must show
//! its effect where d2-sim produces it.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt;

use d2_sim::game::Game;
use d2_sim::tick::timer::{flags, CallbackId, TimerClass, TimerId, TimerList, TimerRun};
use d2_sim::tick::{self, EventDispatch, TickHooks, ROOM_DEACTIVATION_THRESHOLD};
use d2_sim::units::lists::{client_state, HASH_BUCKETS};
use d2_sim::units::{ClientId, RoomId, UnitId, UnitType};
use serde_json::{json, Value};

use crate::Trace;

/// A unit as the trace names it: `[unit type, GUID]`.
type Key = (u8, u32);

/// Tick steps as the trace labels them (`tick.md` §3; `pre` is before
/// the tick, between the previous tick and this one).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Step {
    Pre,
    Env,
    Rooms,
    Events,
    Clients,
    Updq,
    Dels,
    Quests,
    Deact,
    Inactive,
    Items,
    /// After the tick (not a trace label).
    End,
}

impl Step {
    fn parse(s: &str) -> Option<Step> {
        Some(match s {
            "pre" => Step::Pre,
            "env" => Step::Env,
            "rooms" => Step::Rooms,
            "events" => Step::Events,
            "clients" => Step::Clients,
            "updq" => Step::Updq,
            "dels" => Step::Dels,
            "quests" => Step::Quests,
            "deact" => Step::Deact,
            "inactive" => Step::Inactive,
            "items" => Step::Items,
            _ => return None,
        })
    }

    fn name(self) -> &'static str {
        match self {
            Step::Pre => "pre",
            Step::Env => "env",
            Step::Rooms => "rooms",
            Step::Events => "events",
            Step::Clients => "clients",
            Step::Updq => "updq",
            Step::Dels => "dels",
            Step::Quests => "quests",
            Step::Deact => "deact",
            Step::Inactive => "inactive",
            Step::Items => "items",
            Step::End => "end",
        }
    }
}

/// A recorded list primitive.
#[derive(Clone, Debug, PartialEq, Eq)]
enum ListOp {
    HashAdd(Key),
    HashRemove(Key),
    RoomAdd(Key, String),
    RoomRemove(Key),
    QueueAdd(Key, String),
    QueueRemove(Key),
    QueueClear(String),
    RoomActivate(String, u8),
    RoomDeactivate(String, u8),
}

impl ListOp {
    fn unit(&self) -> Option<Key> {
        match self {
            ListOp::HashAdd(k)
            | ListOp::HashRemove(k)
            | ListOp::RoomAdd(k, _)
            | ListOp::RoomRemove(k)
            | ListOp::QueueAdd(k, _)
            | ListOp::QueueRemove(k) => Some(*k),
            _ => None,
        }
    }

    fn kind(&self) -> &'static str {
        match self {
            ListOp::HashAdd(_) => "hash_add",
            ListOp::HashRemove(_) => "hash_remove",
            ListOp::RoomAdd(..) => "room_add",
            ListOp::RoomRemove(_) => "room_remove",
            ListOp::QueueAdd(..) => "queue_add",
            ListOp::QueueRemove(_) => "queue_remove",
            ListOp::QueueClear(_) => "queue_clear",
            ListOp::RoomActivate(..) => "room_activate",
            ListOp::RoomDeactivate(..) => "room_deactivate",
        }
    }
}

impl fmt::Display for ListOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let unit = |k: &Key| format!("[{}, {}]", k.0, k.1);
        match self {
            ListOp::HashAdd(k)
            | ListOp::HashRemove(k)
            | ListOp::RoomRemove(k)
            | ListOp::QueueRemove(k) => write!(f, "{} {}", self.kind(), unit(k)),
            ListOp::RoomAdd(k, r) | ListOp::QueueAdd(k, r) => {
                write!(f, "{} {} {r}", self.kind(), unit(k))
            }
            ListOp::QueueClear(r) => write!(f, "queue_clear {r}"),
            ListOp::RoomActivate(r, a) | ListOp::RoomDeactivate(r, a) => {
                write!(f, "{} {r} act {a}", self.kind())
            }
        }
    }
}

/// A `timer_set` record: what a run of the timer must show.
#[derive(Clone, Debug)]
struct TimerSet {
    timer: u64,
    event: u32,
    unit: Key,
    /// Requested expire (−1: every tick).
    req: i32,
    /// Expire after the §5.2 rule-3 clamp.
    expire: i32,
    a1: u32,
    a2: u32,
    callback: Option<CallbackId>,
}

impl TimerSet {
    fn describe(&self) -> String {
        format!(
            "timer {} (type {}, unit [{}, {}], expire {}, a1 {}, a2 {})",
            self.timer, self.event, self.unit.0, self.unit.1, self.expire, self.a1, self.a2
        )
    }
}

#[derive(Clone, Debug)]
enum Rec {
    TimerSet(TimerSet),
    TimerCancel { timer: u64, deferred: bool },
    List(ListOp),
    TimerRun(u64),
    Lists(Value),
}

#[derive(Clone, Debug)]
struct Ev {
    seq: u64,
    tick: i32,
    step: Step,
    rec: Rec,
}

impl Ev {
    fn describe(&self) -> String {
        match &self.rec {
            Rec::TimerSet(s) => format!("timer_set {}", s.describe()),
            Rec::TimerCancel { timer, deferred } => {
                format!("timer_cancel timer {timer} (deferred {deferred})")
            }
            Rec::List(op) => op.to_string(),
            Rec::TimerRun(t) => format!("timer_run timer {t}"),
            Rec::Lists(_) => "lists snapshot".into(),
        }
    }
}

/// A difference between d2-sim and the recording.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mismatch {
    pub trace: String,
    pub tick: i32,
    /// `seq` of the record where the recording and d2-sim part; 0 when
    /// d2-sim does something after the trace's last record.
    pub seq: u64,
    /// The differing field: a record field (`timer`, `expire`, ...), the
    /// record kind d2-sim produced or missed, or a path in a snapshot
    /// (`acts[0][2].units[0]`).
    pub field: String,
    pub detail: String,
}

impl fmt::Display for Mismatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "trace {}, tick {}, seq {}, {}: {}",
            self.trace, self.tick, self.seq, self.field, self.detail
        )
    }
}

/// Why a tick trace did not replay.
#[derive(Debug, thiserror::Error)]
pub enum ReplayError {
    #[error("trace {id}: {message}")]
    Format { id: String, message: String },
    #[error("{0}")]
    Mismatch(Mismatch),
}

/// What a successful replay compared.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ReplayStats {
    pub ticks: i32,
    pub inputs: usize,
    pub timer_runs: usize,
    pub snapshots: usize,
}

/// Replays a `sim/tick` trace. Returns what was compared, or the first
/// mismatch.
///
/// Not compared: the snapshots' `adj` arrays (adjacent rooms, owned by
/// the DRLG spec, `unit-order.md` §9; no input drives them). Not in the
/// trace, so set up here: the single-player game has one client, in game
/// (`tick.md` §6.4 state 4) from before tick 1; client joins, client
/// states and the client's player link are not recorded, so the client
/// carries no player and d2-sim's per-client player queue insert (§6.5)
/// is replayed from the recording instead of produced.
pub fn replay(trace: &Trace) -> Result<ReplayStats, ReplayError> {
    let format = |message: String| ReplayError::Format {
        id: trace.id.clone(),
        message,
    };
    if (trace.area.as_str(), trace.behavior.as_str()) != ("sim", "tick") {
        return Err(format("not a sim/tick trace".into()));
    }
    if trace.spec != "specs/sim/tick.md" {
        return Err(format(format!(
            "spec {:?} is not the tick spec",
            trace.spec
        )));
    }
    if trace.compare_mode != "exact" || !trace.ignore.is_empty() {
        return Err(format(
            "tick traces compare exactly, nothing ignored".into(),
        ));
    }
    let start = trace.setup["start_frame"]
        .as_i64()
        .and_then(|v| i32::try_from(v).ok())
        .ok_or_else(|| format("setup.start_frame".into()))?;
    let frames = trace.setup["frames"]
        .as_i64()
        .and_then(|v| i32::try_from(v).ok())
        .ok_or_else(|| format("setup.frames".into()))?;
    if start != 0 {
        return Err(format(format!(
            "setup.start_frame {start}: only replays from game creation"
        )));
    }
    if trace.setup["game_type"] != "single player" {
        return Err(format("setup.game_type: only single player".into()));
    }
    let events = parse(trace).map_err(format)?;
    let inputs = trace.inputs.len();

    let mut game = Game::new();
    game.frame = start;
    let mut r = Replay::new(&trace.id, events);
    let client = game.lists.add_client(None, None, client_state::IN_GAME);
    r.clients.insert(client, 1);

    for t in start + 1..=frames + 1 {
        r.tick = t;
        r.step = Step::Pre;
        r.run_pre(&mut game);
        if t > frames {
            break;
        }
        if r.error.is_none() {
            tick::tick(&mut game, &mut r);
        }
        r.enter(&mut game, Step::End);
        if let Some(m) = r.error.take() {
            return Err(ReplayError::Mismatch(m));
        }
        r.stats.ticks += 1;
    }
    if let Some(m) = r.error.take() {
        return Err(ReplayError::Mismatch(m));
    }
    if let Some(ev) = r.events.get(r.pos) {
        return Err(format(format!(
            "seq {} at tick {} lies after the last tick {frames}",
            ev.seq, ev.tick
        )));
    }
    if r.stats.inputs != inputs {
        return Err(format(format!(
            "{} of {inputs} inputs consumed",
            r.stats.inputs
        )));
    }
    Ok(r.stats)
}

/// Merges `inputs` and `expected` into one stream by `seq`.
fn parse(trace: &Trace) -> Result<Vec<Ev>, String> {
    let mut out = Vec::with_capacity(trace.inputs.len() + trace.expected.len());
    for (name, list) in [("inputs", &trace.inputs), ("expected", &trace.expected)] {
        for (i, e) in list.iter().enumerate() {
            out.push(parse_event(e).map_err(|m| format!("{name}[{i}]: {m}"))?);
        }
    }
    out.sort_by_key(|e| e.seq);
    if !out.iter().map(|e| e.seq).eq(1..=out.len() as u64) {
        return Err("seq is not 1..N over inputs and expected".into());
    }
    Ok(out)
}

fn parse_event(e: &Value) -> Result<Ev, String> {
    let d = &e["data"];
    let int = |v: &Value, key: &str| v[key].as_i64().ok_or(format!("missing integer {key}"));
    let i32_of = |key: &str| {
        int(d, key).and_then(|v| i32::try_from(v).map_err(|_| format!("{key} out of range")))
    };
    let u32_of = |key: &str| {
        int(d, key).and_then(|v| u32::try_from(v).map_err(|_| format!("{key} out of range")))
    };
    let u64_of = |key: &str| d[key].as_u64().ok_or(format!("missing integer {key}"));
    let text = |key: &str| {
        d[key]
            .as_str()
            .map(str::to_owned)
            .ok_or(format!("missing string {key}"))
    };
    let unit = || -> Result<Key, String> {
        let u = d["unit"].as_array().ok_or("missing unit")?;
        let ty = u
            .first()
            .and_then(Value::as_u64)
            .and_then(|v| u8::try_from(v).ok());
        let guid = u
            .get(1)
            .and_then(Value::as_u64)
            .and_then(|v| u32::try_from(v).ok());
        match (ty, guid, u.len()) {
            (Some(t), Some(g), 2) if (t as usize) < UnitType::ALL.len() => Ok((t, g)),
            _ => Err(format!("bad unit {}", d["unit"])),
        }
    };
    let act = || int(d, "act").and_then(|a| u8::try_from(a).map_err(|_| "bad act".to_string()));
    let kind = e["kind"].as_str().ok_or("missing kind")?;
    let rec = match kind {
        "timer_set" => {
            let callback = match d.get("cb") {
                None => None,
                Some(cb) => {
                    let s = cb.as_str().ok_or("cb is not a string")?;
                    let hex = s.strip_prefix("0x").ok_or("cb without 0x")?;
                    Some(CallbackId(
                        u32::from_str_radix(hex, 16).map_err(|_| "bad cb")?,
                    ))
                }
            };
            Rec::TimerSet(TimerSet {
                timer: u64_of("timer")?,
                event: u32_of("type")?,
                unit: unit()?,
                req: i32_of("req")?,
                expire: i32_of("expire")?,
                a1: u32_of("a1")?,
                a2: u32_of("a2")?,
                callback,
            })
        }
        "timer_cancel" => Rec::TimerCancel {
            timer: u64_of("timer")?,
            deferred: d["deferred"].as_bool().ok_or("missing deferred")?,
        },
        "timer_run" => Rec::TimerRun(u64_of("timer")?),
        "hash_add" => Rec::List(ListOp::HashAdd(unit()?)),
        "hash_remove" => Rec::List(ListOp::HashRemove(unit()?)),
        "room_add" => Rec::List(ListOp::RoomAdd(unit()?, text("room")?)),
        "room_remove" => Rec::List(ListOp::RoomRemove(unit()?)),
        "queue_add" => Rec::List(ListOp::QueueAdd(unit()?, text("room")?)),
        "queue_remove" => Rec::List(ListOp::QueueRemove(unit()?)),
        "queue_clear" => Rec::List(ListOp::QueueClear(text("room")?)),
        "room_activate" => Rec::List(ListOp::RoomActivate(text("room")?, act()?)),
        "room_deactivate" => Rec::List(ListOp::RoomDeactivate(text("room")?, act()?)),
        "lists" => {
            let mut snap = d.clone();
            if let Some(o) = snap.as_object_mut() {
                o.remove("seq");
                o.remove("step");
            }
            Rec::Lists(snap)
        }
        other => return Err(format!("unknown kind {other:?}")),
    };
    let tick = int(e, "tick").and_then(|t| i32::try_from(t).map_err(|_| "bad tick".into()))?;
    let step_name = d["step"].as_str().ok_or("missing step")?;
    let step = Step::parse(step_name).ok_or(format!("unknown step {step_name:?}"))?;
    Ok(Ev {
        seq: d["seq"].as_u64().ok_or("missing seq")?,
        tick,
        step,
        rec,
    })
}

struct Replay<'t> {
    id: &'t str,
    events: Vec<Ev>,
    /// Next record to consume.
    pos: usize,
    tick: i32,
    step: Step,
    units: BTreeMap<Key, UnitId>,
    /// Units allocated (first named by a timer) but not added yet.
    unlisted: BTreeSet<Key>,
    rooms: BTreeMap<String, RoomId>,
    room_names: BTreeMap<RoomId, String>,
    timers: BTreeMap<u64, TimerId>,
    timer_names: BTreeMap<TimerId, u64>,
    sets: BTreeMap<u64, TimerSet>,
    clients: BTreeMap<ClientId, u64>,
    /// Primitives d2-sim has performed that the recording has not shown
    /// yet, and the d2-sim operation that performed them.
    owed: VecDeque<ListOp>,
    owed_by: &'static str,
    /// The timer whose event is running (trace number, d2-sim id).
    executing: Option<(u64, TimerId)>,
    /// The recording freed the executing timer after its run (seq).
    runner_free: Option<u64>,
    /// The room step 9 was told to deactivate; its `room_deactivate`
    /// record is still to come.
    deact_room: Option<RoomId>,
    error: Option<Mismatch>,
    stats: ReplayStats,
}

impl<'t> Replay<'t> {
    fn new(id: &'t str, events: Vec<Ev>) -> Self {
        Self {
            id,
            events,
            pos: 0,
            tick: 0,
            step: Step::Pre,
            units: BTreeMap::new(),
            unlisted: BTreeSet::new(),
            rooms: BTreeMap::new(),
            room_names: BTreeMap::new(),
            timers: BTreeMap::new(),
            timer_names: BTreeMap::new(),
            sets: BTreeMap::new(),
            clients: BTreeMap::new(),
            owed: VecDeque::new(),
            owed_by: "",
            executing: None,
            runner_free: None,
            deact_room: None,
            error: None,
            stats: ReplayStats::default(),
        }
    }

    fn failed(&self) -> bool {
        self.error.is_some()
    }

    fn fail(&mut self, seq: u64, field: &str, detail: String) {
        if self.error.is_none() {
            self.error = Some(Mismatch {
                trace: self.id.to_owned(),
                tick: self.tick,
                seq,
                field: field.to_owned(),
                detail,
            });
        }
    }

    /// The next record if it belongs to the current tick and `step`.
    fn peek_in(&self, step: Step) -> Option<&Ev> {
        self.events
            .get(self.pos)
            .filter(|e| e.tick == self.tick && e.step == step)
    }

    fn next_seq(&self) -> u64 {
        self.events.get(self.pos).map_or(0, |e| e.seq)
    }

    fn next_desc(&self) -> String {
        match self.events.get(self.pos) {
            Some(e) => format!(
                "the recording's next record is {} (tick {}, step {})",
                e.describe(),
                e.tick,
                e.step.name()
            ),
            None => "the recording has ended".into(),
        }
    }

    // ---- steps ---------------------------------------------------------

    /// Records before the tick: inputs applied, snapshots compared.
    fn run_pre(&mut self, game: &mut Game) {
        while !self.failed() {
            let Some(ev) = self.peek_in(Step::Pre).cloned() else {
                break;
            };
            self.pos += 1;
            match &ev.rec {
                Rec::Lists(want) => {
                    self.check_owed(ev.seq);
                    self.compare_snapshot(game, ev.seq, want);
                }
                Rec::TimerRun(_) => self.fail(
                    ev.seq,
                    "timer_run",
                    "recorded timer run outside the event step".into(),
                ),
                _ => self.apply(game, &ev),
            }
        }
        self.check_owed(self.next_seq());
    }

    /// Called by every hook: closes the steps before `step` and, on
    /// entering a step whose recorded inputs have no finer place, applies
    /// them all.
    fn enter(&mut self, game: &mut Game, step: Step) {
        if self.failed() || step == self.step {
            return;
        }
        if step < self.step {
            let detail = format!(
                "hook of step {} after step {}",
                step.name(),
                self.step.name()
            );
            self.fail(self.next_seq(), "step", detail);
            return;
        }
        self.close_step(game);
        if self.failed() {
            return;
        }
        // Anything still recorded for an earlier step of this tick had
        // no place in d2-sim's tick.
        if let Some(ev) = self.events.get(self.pos) {
            if ev.tick == self.tick && ev.step < step {
                let (field, detail) = match &ev.rec {
                    Rec::TimerRun(t) => (
                        "timer_run",
                        format!(
                            "recorded run of {}; d2-sim's queue run ended without it",
                            self.timer_desc(*t)
                        ),
                    ),
                    _ => (
                        "step",
                        format!(
                            "recorded {} in step {}; d2-sim's step {} gave it no place",
                            ev.describe(),
                            ev.step.name(),
                            ev.step.name()
                        ),
                    ),
                };
                let seq = ev.seq;
                self.fail(seq, field, detail);
                return;
            }
        }
        self.step = step;
        if !matches!(step, Step::Events | Step::Deact | Step::End) {
            self.apply_step(game, step);
        }
    }

    fn close_step(&mut self, game: &mut Game) {
        match self.step {
            Step::Events => self.settle_run(game),
            Step::Deact => self.settle_deact(game),
            _ => {}
        }
        self.check_owed(self.next_seq());
    }

    /// Applies every recorded input of `step` in this tick.
    fn apply_step(&mut self, game: &mut Game, step: Step) {
        while !self.failed() {
            let Some(ev) = self.peek_in(step).cloned() else {
                break;
            };
            if matches!(ev.rec, Rec::TimerRun(_) | Rec::Lists(_)) {
                break;
            }
            self.pos += 1;
            self.apply(game, &ev);
        }
    }

    fn check_owed(&mut self, seq: u64) {
        if let Some(op) = self.owed.front() {
            let detail = format!(
                "d2-sim's {} also did {op}; {}",
                self.owed_by,
                self.next_desc()
            );
            let kind = op.kind();
            self.fail(seq, kind, detail);
        }
    }

    // ---- inputs ----------------------------------------------------------

    fn apply(&mut self, game: &mut Game, ev: &Ev) {
        match &ev.rec {
            Rec::TimerSet(s) => self.timer_set(game, ev.seq, s),
            Rec::TimerCancel { timer, deferred } => {
                self.timer_cancel(game, ev.seq, *timer, *deferred)
            }
            Rec::List(op) => self.list_op(game, ev, op),
            Rec::TimerRun(_) | Rec::Lists(_) => unreachable!("not an input"),
        }
        self.stats.inputs += 1;
    }

    fn timer_set(&mut self, game: &mut Game, seq: u64, s: &TimerSet) {
        // A unit's type init may schedule timers before `SUNIT_Add`
        // (unit-order.md §1.4, §3.1); allocation itself is not recorded,
        // so a unit first named by a timer is allocated here.
        let unit = match self.units.get(&s.unit) {
            Some(&id) => id,
            None => match self.create_unit(game, seq, s.unit, None) {
                Some(id) => id,
                None => return,
            },
        };
        if self.timers.contains_key(&s.timer) || self.sets.contains_key(&s.timer) {
            self.fail(seq, "timer", format!("timer {} scheduled twice", s.timer));
            return;
        }
        let id = match game.schedule_event(unit, s.event, s.req, s.callback, s.a1, s.a2) {
            Ok(Some(id)) => id,
            Ok(None) => {
                let detail = format!("d2-sim does not schedule event type {}", s.event);
                self.fail(seq, "type", detail);
                return;
            }
            Err(e) => {
                self.fail(seq, "unit", format!("d2-sim: {e}"));
                return;
            }
        };
        let expire = game.timers.expire(id);
        if expire != Some(s.expire) {
            let detail = format!(
                "requested {} at frame {}: d2-sim gives {expire:?}, recorded {}",
                s.req, game.frame, s.expire
            );
            self.fail(seq, "expire", detail);
            return;
        }
        self.timers.insert(s.timer, id);
        self.timer_names.insert(id, s.timer);
        self.sets.insert(s.timer, s.clone());
    }

    fn timer_cancel(&mut self, game: &mut Game, seq: u64, timer: u64, deferred: bool) {
        let Some(&id) = self.timers.get(&timer) else {
            self.fail(
                seq,
                "timer",
                format!("cancel of timer {timer}, not live in d2-sim"),
            );
            return;
        };
        let executing = self.executing.map(|(t, _)| t) == Some(timer);
        if deferred != executing {
            let detail = format!(
                "recorded deferred {deferred} for timer {timer}; d2-sim's executing timer is {:?}",
                self.executing.map(|(t, _)| t)
            );
            self.fail(seq, "deferred", detail);
            return;
        }
        game.timers.cancel(id);
        let flags = game.timers.flags(id);
        if deferred {
            // §5.4 rule 1: only marked; the runner frees it.
            if flags.is_none_or(|f| f & flags::DELETE == 0) {
                let detail = format!("d2-sim does not defer cancelling executing timer {timer}");
                self.fail(seq, "deferred", detail);
            }
        } else if flags.is_some() {
            self.fail(
                seq,
                "timer_cancel",
                format!("d2-sim keeps timer {timer} after cancel"),
            );
        } else {
            self.forget_timer(timer, id);
        }
    }

    fn forget_timer(&mut self, timer: u64, id: TimerId) {
        self.timers.remove(&timer);
        self.timer_names.remove(&id);
    }

    fn room_id(&mut self, seq: u64, name: &str) -> Option<RoomId> {
        let id = self.rooms.get(name).copied();
        if id.is_none() {
            self.fail(seq, "room", format!("room {name} is not active in d2-sim"));
        }
        id
    }

    fn unit_id(&mut self, seq: u64, k: Key) -> Option<UnitId> {
        let id = self.units.get(&k).copied();
        if id.is_none() {
            self.fail(
                seq,
                "unit",
                format!("unit [{}, {}] not in d2-sim", k.0, k.1),
            );
        }
        id
    }

    fn key_of(&self, game: &Game, id: UnitId) -> Key {
        let e = game.lists.unit(id).expect("unit in d2-sim");
        (e.ty as u8, e.guid)
    }

    fn room_name(&self, room: Option<RoomId>) -> String {
        room.and_then(|r| self.room_names.get(&r).cloned())
            .unwrap_or_else(|| "none".into())
    }

    /// Whether the unit's next list primitives in this step, after its
    /// queue unlink, remove it from its hash list (unit removal,
    /// unit-order.md §3.2) rather than leave it roomless or move it.
    fn removal_follows(&self, k: Key) -> bool {
        for ev in &self.events[self.pos..] {
            if ev.tick != self.tick || ev.step != self.step {
                break;
            }
            match &ev.rec {
                Rec::List(op) if op.unit() == Some(k) => match op {
                    ListOp::QueueRemove(_) => {}
                    ListOp::HashRemove(_) => return true,
                    _ => return false,
                },
                _ => {}
            }
        }
        false
    }

    /// Creates a unit: allocated and added (`SUNIT_Add`, unit-order.md
    /// §3.1) with `add = Some(room)`, only allocated (§1.4) with `None`.
    /// A GUID above the type's counter is a fresh allocation (§1.3) and
    /// must be the one d2-sim allocates; a lower one is a restored unit
    /// keeping its GUID (§1.4).
    fn create_unit(
        &mut self,
        game: &mut Game,
        seq: u64,
        k: Key,
        add: Option<Option<RoomId>>,
    ) -> Option<UnitId> {
        let ty = UnitType::ALL[k.0 as usize];
        let allied = ty == UnitType::Player;
        let result = if k.1 > game.lists.guids.get(ty) {
            let next = game.lists.guids.get(ty).wrapping_add(1);
            if next != k.1 {
                let detail = format!(
                    "d2-sim allocates GUID {next} for type {}, recorded {}",
                    k.0, k.1
                );
                self.fail(seq, "guid", detail);
                return None;
            }
            match add {
                Some(room) => game.spawn_unit(ty, room, allied).map_err(|e| e.to_string()),
                None => Ok(game.alloc_unit(ty, allied)),
            }
        } else {
            match add {
                Some(room) => game
                    .lists
                    .add_unit(ty, k.1, room, allied)
                    .map_err(|e| e.to_string()),
                None => Ok(game.lists.alloc_unit(ty, k.1, allied)),
            }
        };
        match result {
            Ok(id) => {
                self.units.insert(k, id);
                if add.is_none() {
                    self.unlisted.insert(k);
                }
                Some(id)
            }
            Err(e) => {
                self.fail(seq, "unit", format!("d2-sim: {e}"));
                None
            }
        }
    }

    /// One recorded list primitive: either the next one owed by the last
    /// d2-sim operation, or the start of a new one.
    fn list_op(&mut self, game: &mut Game, ev: &Ev, op: &ListOp) {
        let seq = ev.seq;
        if let Some(front) = self.owed.pop_front() {
            if &front != op {
                let detail = format!("d2-sim's {} did {front} here, recorded {op}", self.owed_by);
                self.fail(seq, op.kind(), detail);
            }
            return;
        }
        let (by, footprint): (&'static str, Vec<ListOp>) = match op {
            ListOp::RoomAdd(k, name) => {
                let Some(room) = self.room_id(seq, name) else {
                    return;
                };
                match self.units.get(k).copied() {
                    Some(id) if self.unlisted.contains(k) => {
                        if let Err(e) = game.lists.add_allocated(id, Some(room)) {
                            self.fail(seq, "room_add", format!("d2-sim: {e}"));
                            return;
                        }
                        self.unlisted.remove(k);
                        let mut fp = vec![op.clone()];
                        if game.lists.unit(id).is_some_and(|e| e.is_queued()) {
                            fp.push(ListOp::QueueAdd(*k, name.clone()));
                        }
                        fp.push(ListOp::HashAdd(*k));
                        ("add_allocated", fp)
                    }
                    None => {
                        let Some(id) = self.create_unit(game, seq, *k, Some(Some(room))) else {
                            return;
                        };
                        let mut fp = vec![op.clone()];
                        if game.lists.unit(id).is_some_and(|e| e.is_queued()) {
                            fp.push(ListOp::QueueAdd(*k, name.clone()));
                        }
                        fp.push(ListOp::HashAdd(*k));
                        ("add_unit", fp)
                    }
                    Some(id) => {
                        let was = game.lists.unit(id).is_some_and(|e| e.is_queued());
                        if let Err(e) = game.lists.room_insert(id, room) {
                            self.fail(seq, "room_add", format!("d2-sim: {e}"));
                            return;
                        }
                        let mut fp = vec![op.clone()];
                        if !was && game.lists.unit(id).is_some_and(|e| e.is_queued()) {
                            fp.push(ListOp::QueueAdd(*k, name.clone()));
                        }
                        ("room_insert", fp)
                    }
                }
            }
            ListOp::HashAdd(k) if self.unlisted.contains(k) => {
                let id = self.units[k];
                if let Err(e) = game.lists.add_allocated(id, None) {
                    self.fail(seq, "hash_add", format!("d2-sim: {e}"));
                    return;
                }
                self.unlisted.remove(k);
                ("add_allocated", vec![op.clone()])
            }
            ListOp::HashAdd(k) => {
                if self.units.contains_key(k) {
                    let detail = format!("unit [{}, {}] is already in d2-sim", k.0, k.1);
                    self.fail(seq, "hash_add", detail);
                    return;
                }
                if self.create_unit(game, seq, *k, Some(None)).is_none() {
                    return;
                }
                ("add_unit", vec![op.clone()])
            }
            ListOp::RoomRemove(k) => {
                let Some(id) = self.unit_id(seq, *k) else {
                    return;
                };
                let had_room = game.lists.unit(id).is_some_and(|e| e.room().is_some());
                let mut fp = vec![op.clone()];
                if had_room {
                    fp.push(ListOp::QueueRemove(*k));
                }
                let removal = self.removal_follows(*k);
                if removal {
                    if let Err(e) = game.lists.remove_unit(id) {
                        self.fail(seq, "room_remove", format!("d2-sim: {e}"));
                        return;
                    }
                    self.units.remove(k);
                    self.unlisted.remove(k);
                    fp.push(ListOp::HashRemove(*k));
                    ("remove_unit", fp)
                } else {
                    if let Err(e) = game.lists.room_remove(id) {
                        self.fail(seq, "room_remove", format!("d2-sim: {e}"));
                        return;
                    }
                    ("room_remove", fp)
                }
            }
            ListOp::HashRemove(k) => {
                let detail = format!(
                    "recorded hash removal of [{}, {}] without its room unlink; d2-sim's \
                     unit removal (unit-order.md §3.2) starts with room_remove",
                    k.0, k.1
                );
                self.fail(seq, "hash_remove", detail);
                return;
            }
            ListOp::QueueAdd(k, _) => {
                let Some(id) = self.unit_id(seq, *k) else {
                    return;
                };
                let was = game.lists.unit(id).is_some_and(|e| e.is_queued());
                if let Err(e) = game.lists.queue_update(id) {
                    self.fail(seq, "queue_add", format!("d2-sim: {e}"));
                    return;
                }
                let e = game.lists.unit(id).expect("unit");
                if was || !e.is_queued() {
                    let detail = format!(
                        "d2-sim does not queue [{}, {}] (queued before: {was}, room {})",
                        k.0,
                        k.1,
                        self.room_name(e.room())
                    );
                    self.fail(seq, "queue_add", detail);
                    return;
                }
                let room = self.room_name(e.room());
                ("queue_update", vec![ListOp::QueueAdd(*k, room)])
            }
            ListOp::QueueRemove(k) => {
                let Some(id) = self.unit_id(seq, *k) else {
                    return;
                };
                if let Err(e) = game.lists.unqueue_update(id) {
                    self.fail(seq, "queue_remove", format!("d2-sim: {e}"));
                    return;
                }
                ("unqueue_update", vec![op.clone()])
            }
            ListOp::QueueClear(name) => {
                let Some(room) = self.room_id(seq, name) else {
                    return;
                };
                if let Err(e) = game.lists.clear_update_queue(room) {
                    self.fail(seq, "queue_clear", format!("d2-sim: {e}"));
                    return;
                }
                ("clear_update_queue", vec![op.clone()])
            }
            ListOp::RoomActivate(name, act) => {
                if self.rooms.contains_key(name) {
                    self.fail(seq, "room", format!("room {name} activated twice"));
                    return;
                }
                let made = game
                    .lists
                    .ensure_act(*act)
                    .and_then(|()| game.lists.create_room(*act))
                    .and_then(|id| game.lists.activate_room(id).map(|()| id));
                match made {
                    Ok(id) => {
                        self.rooms.insert(name.clone(), id);
                        self.room_names.insert(id, name.clone());
                    }
                    Err(e) => {
                        self.fail(seq, "room_activate", format!("d2-sim: {e}"));
                        return;
                    }
                }
                ("activate_room", vec![op.clone()])
            }
            ListOp::RoomDeactivate(name, act) => {
                let Some(room) = self.room_id(seq, name) else {
                    return;
                };
                let room_act = game.lists.room(room).map(|r| r.act);
                if room_act != Some(*act) {
                    let detail = format!("room {name} is in act {room_act:?}, recorded act {act}");
                    self.fail(seq, "act", detail);
                    return;
                }
                if let Err(e) = game.lists.deactivate_room(room) {
                    self.fail(seq, "room_deactivate", format!("d2-sim: {e}"));
                    return;
                }
                self.rooms.remove(name);
                ("deactivate_room", vec![op.clone()])
            }
        };
        if footprint.first() != Some(op) {
            let detail = format!(
                "d2-sim's {by} starts with {:?}, recorded {op}",
                footprint.first()
            );
            self.fail(seq, op.kind(), detail);
            return;
        }
        self.owed = footprint.into_iter().skip(1).collect();
        self.owed_by = by;
    }

    // ---- timer runs (step 4) ---------------------------------------------

    fn timer_desc(&self, t: u64) -> String {
        self.sets
            .get(&t)
            .map_or_else(|| format!("timer {t}"), TimerSet::describe)
    }

    fn run_desc(&self, run: &TimerRun) -> String {
        let name = self
            .timer_names
            .get(&run.timer)
            .map_or_else(|| "?".to_string(), u64::to_string);
        format!(
            "timer {name} (type {}, unit [{}, {}], expire {}, a1 {}, a2 {})",
            run.event, run.owner.unit_type as u8, run.owner.guid, run.expire, run.arg1, run.arg2
        )
    }

    /// After a run: the runner frees a due timer, and an every-tick one
    /// only if it was cancelled during its event (§5.5 rule 3). The
    /// recording logs the free as a cancel right after the run.
    fn settle_run(&mut self, game: &mut Game) {
        let Some((t, id)) = self.executing.take() else {
            return;
        };
        let freed = game.timers.flags(id).is_none();
        match (self.runner_free.take(), freed) {
            (Some(_), true) => self.forget_timer(t, id),
            (None, false) => {}
            (Some(seq), false) => self.fail(
                seq,
                "timer_cancel",
                format!("the recording frees timer {t} after its run; d2-sim keeps it"),
            ),
            (None, true) => {
                let detail = format!(
                    "d2-sim frees timer {t} after its run; the recording does not ({})",
                    self.next_desc()
                );
                self.fail(self.next_seq(), "timer_cancel", detail);
            }
        }
    }

    fn compare_run(&mut self, seq: u64, recorded: u64, run: &TimerRun) {
        let ours = self.timer_names.get(&run.timer).copied();
        if ours != Some(recorded) {
            let detail = format!(
                "d2-sim runs {}, recorded {}",
                self.run_desc(run),
                self.timer_desc(recorded)
            );
            self.fail(seq, "timer", detail);
            return;
        }
        let s = &self.sets[&recorded];
        let ty = UnitType::ALL[s.unit.0 as usize];
        let list = if s.req == -1 {
            TimerList::EveryTick
        } else {
            TimerList::Due
        };
        let checks: [(&str, bool); 8] = [
            ("class", TimerClass::of(ty) == Some(run.class)),
            ("list", list == run.list),
            ("type", u32::from(run.event) == s.event),
            (
                "unit",
                (run.owner.unit_type, run.owner.guid) == (ty, s.unit.1),
            ),
            ("expire", run.expire == s.expire),
            ("a1", run.arg1 == s.a1),
            ("a2", run.arg2 == s.a2),
            ("cb", run.callback == s.callback),
        ];
        if let Some((field, _)) = checks.iter().find(|(_, ok)| !ok) {
            let detail = format!(
                "d2-sim runs {}, recorded {}",
                self.run_desc(run),
                s.describe()
            );
            self.fail(seq, field, detail);
        }
    }

    // ---- step 9 ------------------------------------------------------------

    /// The room d2-sim was told to deactivate must be gone from its act
    /// list, and the recording's next list record must say so.
    fn settle_deact(&mut self, game: &mut Game) {
        let Some(room) = self.deact_room.take() else {
            return;
        };
        let name = self.room_name(Some(room));
        while !self.failed() {
            let Some(ev) = self.peek_in(Step::Deact).cloned() else {
                break;
            };
            match &ev.rec {
                Rec::List(ListOp::RoomDeactivate(r, act)) if *r == name => {
                    self.pos += 1;
                    let entry = game.lists.room(room).expect("room");
                    if entry.is_active() {
                        let detail = format!("the recording deactivates {name}; d2-sim keeps it");
                        self.fail(ev.seq, "room_deactivate", detail);
                    } else if entry.act != *act {
                        let detail = format!("room {name} is in act {}, recorded {act}", entry.act);
                        self.fail(ev.seq, "act", detail);
                    } else {
                        self.rooms.remove(&name);
                        self.stats.inputs += 1;
                    }
                    return;
                }
                Rec::TimerSet(_) | Rec::TimerCancel { .. } => {
                    self.pos += 1;
                    self.apply(game, &ev);
                }
                _ => break,
            }
        }
        let detail = format!("d2-sim deactivates room {name} here; {}", self.next_desc());
        self.fail(self.next_seq(), "room_deactivate", detail);
    }

    /// Whether the recording deactivates `room` in this tick's step 9.
    fn deactivated_here(&self, room: RoomId) -> bool {
        let Some(name) = self.room_names.get(&room) else {
            return false;
        };
        self.events[self.pos..]
            .iter()
            .take_while(|e| e.tick == self.tick && e.step == Step::Deact)
            .any(|e| matches!(&e.rec, Rec::List(ListOp::RoomDeactivate(r, _)) if r == name))
    }

    // ---- snapshots ---------------------------------------------------------

    fn snapshot(&self, game: &Game) -> Value {
        let unit = |id: &UnitId| {
            let e = game.lists.unit(*id).expect("listed unit");
            json!([e.ty as u8, e.guid])
        };
        let guids = |ids: Vec<UnitId>| -> Vec<u32> {
            ids.iter()
                .map(|id| game.lists.unit(*id).expect("listed unit").guid)
                .collect()
        };
        let mut hash = serde_json::Map::new();
        for ty in &UnitType::ALL[..5] {
            let buckets: Vec<Value> = (0..HASH_BUCKETS)
                .filter_map(|b| {
                    let g = guids(game.lists.hash_bucket(*ty, b));
                    (!g.is_empty()).then(|| json!([b, g]))
                })
                .collect();
            hash.insert((*ty as u8).to_string(), Value::Array(buckets));
        }
        let acts: Vec<Value> = (0..5u8)
            .map(|a| match game.lists.act(a) {
                None => Value::Null,
                Some(_) => game
                    .lists
                    .active_rooms(a)
                    .iter()
                    .map(|r| {
                        json!({
                            "room": self.room_name(Some(*r)),
                            "units": game.lists.room_units(*r).iter().map(unit).collect::<Vec<_>>(),
                            "queue": game.lists.update_queue(*r).iter().map(unit).collect::<Vec<_>>(),
                        })
                    })
                    .collect(),
            })
            .collect();
        let clients: Vec<Value> = game
            .lists
            .clients()
            .iter()
            .map(|c| self.clients.get(c).map_or(Value::Null, |n| json!(n)))
            .collect();
        json!({
            "hash": hash,
            "tiles": guids(game.lists.hash_bucket(UnitType::Tile, 0)),
            "acts": acts,
            "clients": clients,
        })
    }

    fn compare_snapshot(&mut self, game: &Game, seq: u64, want: &Value) {
        let mut want = want.clone();
        // Adjacent-room arrays: DRLG data, not driven by any input.
        if let Some(acts) = want["acts"].as_array_mut() {
            for rooms in acts.iter_mut().filter_map(Value::as_array_mut) {
                for room in rooms.iter_mut().filter_map(Value::as_object_mut) {
                    room.remove("adj");
                }
            }
        }
        let got = self.snapshot(game);
        if let Some((path, g, w)) = first_diff("", &got, &want) {
            self.fail(seq, &path, format!("d2-sim {g}, recorded {w}"));
        } else {
            self.stats.snapshots += 1;
        }
    }
}

/// The first path where `got` and `want` differ, with both values there.
fn first_diff(path: &str, got: &Value, want: &Value) -> Option<(String, String, String)> {
    if got == want {
        return None;
    }
    let short = |v: &Value| {
        let s = v.to_string();
        if s.len() > 160 {
            format!("{}...", &s[..160])
        } else {
            s
        }
    };
    match (got, want) {
        (Value::Array(a), Value::Array(b)) => {
            for (i, (x, y)) in a.iter().zip(b).enumerate() {
                if let Some(d) = first_diff(&format!("{path}[{i}]"), x, y) {
                    return Some(d);
                }
            }
            let detail = |v: &Vec<Value>| format!("{} entries {}", v.len(), short(&json!(v)));
            Some((path.to_owned(), detail(a), detail(b)))
        }
        (Value::Object(a), Value::Object(b)) => {
            let keys: BTreeSet<&String> = a.keys().chain(b.keys()).collect();
            for k in keys {
                let p = if path.is_empty() {
                    k.clone()
                } else {
                    format!("{path}.{k}")
                };
                let (x, y) = (
                    a.get(k).unwrap_or(&Value::Null),
                    b.get(k).unwrap_or(&Value::Null),
                );
                if let Some(d) = first_diff(&p, x, y) {
                    return Some(d);
                }
            }
            None
        }
        _ => Some((path.to_owned(), short(got), short(want))),
    }
}

impl EventDispatch for Replay<'_> {
    /// Compares d2-sim's run with the next recorded one, then applies what
    /// the recorded event did (its schedules, cancels and list changes).
    fn run_event(&mut self, game: &mut Game, run: &TimerRun) {
        self.enter(game, Step::Events);
        self.settle_run(game);
        if self.failed() {
            return;
        }
        let next = self.peek_in(Step::Events).cloned();
        let Some(Ev {
            seq,
            rec: Rec::TimerRun(recorded),
            ..
        }) = next
        else {
            let detail = format!("d2-sim runs {}; {}", self.run_desc(run), self.next_desc());
            self.fail(self.next_seq(), "timer_run", detail);
            return;
        };
        self.pos += 1;
        self.compare_run(seq, recorded, run);
        if self.failed() {
            return;
        }
        self.stats.timer_runs += 1;
        self.executing = Some((recorded, run.timer));
        while !self.failed() {
            let Some(ev) = self.peek_in(Step::Events).cloned() else {
                break;
            };
            match ev.rec {
                Rec::TimerRun(_) | Rec::Lists(_) => break,
                Rec::TimerCancel {
                    timer,
                    deferred: false,
                } if timer == recorded => {
                    // The runner's free after the event (§5.5 rule 3):
                    // d2-sim's own work, checked by `settle_run`.
                    self.pos += 1;
                    self.stats.inputs += 1;
                    self.runner_free = Some(ev.seq);
                    if let Some(after) = self.peek_in(Step::Events) {
                        if !matches!(after.rec, Rec::TimerRun(_)) {
                            let detail = format!(
                                "recorded {} after the runner freed timer {recorded}",
                                after.describe()
                            );
                            let seq = after.seq;
                            self.fail(seq, "timer_cancel", detail);
                        }
                    }
                    break;
                }
                _ => {
                    self.pos += 1;
                    self.apply(game, &ev);
                }
            }
        }
        self.check_owed(self.next_seq());
    }
}

impl TickHooks for Replay<'_> {
    fn advance_environment(&mut self, game: &mut Game, _act: u8) -> bool {
        self.enter(game, Step::Env);
        false
    }

    fn environment_changed(&mut self, game: &mut Game, _act: u8, _client: ClientId) {
        self.enter(game, Step::Env);
    }

    fn ambient_spawns(&mut self, game: &mut Game, _room: RoomId) {
        self.enter(game, Step::Rooms);
    }

    fn spawn_presets(&mut self, game: &mut Game, _room: RoomId) {
        self.enter(game, Step::Rooms);
    }

    fn restore_inactive_units(&mut self, game: &mut Game, _room: RoomId) {
        self.enter(game, Step::Rooms);
    }

    fn populate_objects(&mut self, game: &mut Game, _room: RoomId) {
        self.enter(game, Step::Rooms);
    }

    fn populate_monsters(&mut self, game: &mut Game, _room: RoomId) {
        self.enter(game, Step::Rooms);
    }

    fn client_room_ready(&mut self, game: &mut Game, _client: ClientId) -> bool {
        self.enter(game, Step::Clients);
        false
    }

    fn send_load_complete(&mut self, game: &mut Game, _client: ClientId) {
        self.enter(game, Step::Clients);
    }

    fn refresh_inventory(&mut self, game: &mut Game, _client: ClientId) {
        self.enter(game, Step::Clients);
    }

    fn join_sequence(&mut self, game: &mut Game, _client: ClientId) {
        self.enter(game, Step::Clients);
    }

    fn send_removed_units(&mut self, game: &mut Game, _client: ClientId) {
        self.enter(game, Step::Clients);
    }

    fn send_unit_update(&mut self, game: &mut Game, _client: ClientId, _unit: UnitId) {
        self.enter(game, Step::Clients);
    }

    fn client_update_messages(&mut self, game: &mut Game, _client: ClientId) {
        self.enter(game, Step::Clients);
    }

    fn client_level_change(&mut self, game: &mut Game, _client: ClientId) {
        self.enter(game, Step::Clients);
    }

    fn arena_sync(&mut self, game: &mut Game, _client: ClientId) {
        self.enter(game, Step::Clients);
    }

    fn unit_update(&mut self, game: &mut Game, _unit: UnitId) {
        self.enter(game, Step::Updq);
    }

    fn clear_arena_flag(&mut self, game: &mut Game) {
        self.enter(game, Step::Updq);
    }

    fn free_removal_records(&mut self, game: &mut Game, _room: RoomId) {
        self.enter(game, Step::Dels);
    }

    fn update_quests(&mut self, game: &mut Game) {
        self.enter(game, Step::Quests);
    }

    /// Step 9 asks per active room, in act room-list order; the room
    /// counts as inactive (tick.md open question 4) when the recording
    /// deactivates it in this tick.
    fn room_inactivity(&mut self, game: &mut Game, room: RoomId) -> u32 {
        self.enter(game, Step::Deact);
        self.settle_deact(game);
        if self.failed() || !self.deactivated_here(room) {
            return 0;
        }
        self.deact_room = Some(room);
        ROOM_DEACTIVATION_THRESHOLD + 1
    }

    fn act_allows_room_removal(&mut self, game: &mut Game, _act: u8, room: RoomId) -> bool {
        self.enter(game, Step::Deact);
        self.deact_room == Some(room)
    }

    /// Compressing a unit (`0x005433F0`): applies the recording's records
    /// up to the unit's removal, which must come next.
    fn compress_unit(&mut self, game: &mut Game, unit: UnitId) {
        self.enter(game, Step::Deact);
        if self.failed() {
            return;
        }
        let k = self.key_of(game, unit);
        let mut started = false;
        while !self.failed() {
            if started && self.owed.is_empty() {
                return;
            }
            let Some(ev) = self.peek_in(Step::Deact).cloned() else {
                break;
            };
            match &ev.rec {
                Rec::TimerSet(_) | Rec::TimerCancel { .. } => {}
                Rec::List(op) if started || *op == ListOp::RoomRemove(k) => started = true,
                _ => break,
            }
            self.pos += 1;
            self.apply(game, &ev);
        }
        if !self.failed() {
            let detail = format!(
                "d2-sim compresses unit [{}, {}] of room {}; {}",
                k.0,
                k.1,
                self.room_name(self.deact_room),
                self.next_desc()
            );
            self.fail(self.next_seq(), "room_remove", detail);
        }
    }

    fn free_inactive_rooms(&mut self, game: &mut Game, _act: u8) {
        self.enter(game, Step::Inactive);
    }

    fn delete_inactive_items(&mut self, game: &mut Game, _act: u8) {
        self.enter(game, Step::Items);
    }

    fn expire_inactive_unit_items(&mut self, game: &mut Game, _act: u8) {
        self.enter(game, Step::Items);
    }
}
