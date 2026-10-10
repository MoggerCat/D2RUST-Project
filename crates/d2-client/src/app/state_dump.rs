// Spec: specs/tools/state-snapshot.md (§1, §3)
//! `d2-client state-dump`: the game `d2-client play --save X --seed N`
//! builds, run headless (no window, no Bevy) on a stepping clock, with
//! one `state-1` snapshot (`d2_sim::debug::state`) written after every
//! server tick that ran (or every n-th frame).
//!
//! The run: the game is built as `play` builds it
//! ([`super::play_start::resolve`] for `--save` / `--difficulty`,
//! [`single_player::game_seed`] for `--seed`,
//! [`single_player::start_with`], the hardcore switch), the client is the
//! plain [`Bridge`] (`play`'s link handler without the Bevy app) with the
//! bridge-level client data `play` installs (client DRLG source, level,
//! object, skill and unit rows). The bridge sends the 0x67 create request
//! before the first pump and answers 0x02 with 0x6B on its own, as in
//! `play`. Each step advances the clock 40 ms and runs one bridge frame
//! (pump: drain → tick → flush; receive; the model's own answers). After
//! a frame whose pump ticked, the snapshot of §3 rule 1 (the state after
//! tick N, `f` = `Game::frame` = N) is taken on the server thread
//! through [`ThreadLink::with`], reading only.
//!
//! Pokes (`specs/tools/poke.md` §2 rule 6): each `--poke "<f> <directive>
//! <args>..."` runs through the bridge ([`Bridge::poke`], on the server
//! thread) right after the snapshot of frame f − 1, before frame f's
//! drain; its result is a `poke` line between the two snapshots (the
//! record `poke.py` writes on 1.14d) and a line on stderr.
//!
//! Input (`specs/tools/scenario-diff.md` §2 r4, §3 r8): `--input SCRIPT`,
//! the shared frame-anchored form (`frame F; click X Y; hold X Y N; move X
//! Y`), runs at the same point as the pokes, after them: each due step's
//! pointer events go through the bridge's world-click dispatcher
//! ([`Headless`]); its log lines go to stderr and the footer notes.
//!
//! Sends (`specs/tools/scenario-diff.md` §2 `at … send`): each `--send
//! "<f> <Name> <field>=<value>..."` or `--send "<f> hex <byte>..."` is
//! injected through the bridge ([`Bridge::inject`]: references resolved
//! on the server game, bytes handed to the transport send after the
//! duplicate filter, `specs/tools/scenario.md` §4 r2 (a)) at the same
//! point, after the pokes and the input, in command-line order; each
//! result is a `send` line (bytes or unresolved), a line on stderr and a
//! footer note.
//!
//! Output: header, snaps (and poke / send lines), footer (§1). Two runs with the same arguments
//! write the same bytes but for the header's `date`. Not game logic: the
//! clock and the date are this binary's (CLAUDE.md rule 6 binds
//! `d2-sim`, which only reads here).

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use anyhow::{bail, Context, Result};
use d2_server::seams::Clock;
use d2_sim::debug::state;

use super::play_start::{self, CliStart};
use super::poke::{self as pokes, Entry, When};
use super::send::{self as sends, SendEntry};
use super::server_thread::ThreadLink;
use super::single_player::{self, Character, GameData, Link};
use crate::bridge::output::{Consumer, Output};
use crate::bridge::predict::PredictLink;
use crate::bridge::state::StateSource;
use crate::bridge::Bridge;
use crate::world_view::input_script::{self, Headless};
use d2_sim::poke::{Directive, GotoWalk, PokeOp, PokeResult};

/// Milliseconds the clock advances per step: one server tick (25 Hz).
pub const STEP_MS: u32 = 40;

/// The clock's value at the first pump (as the headless app tests).
pub const START_MS: u32 = 1000;

/// Steps without a tick after which the run stops (the server is stuck).
const MAX_IDLE_STEPS: u32 = 1000;

/// A host clock the dump steps by hand.
#[derive(Clone, Debug, Default)]
pub struct StepClock(pub Arc<AtomicU32>);

impl Clock for StepClock {
    fn now_ms(&mut self) -> u32 {
        self.0.load(Ordering::SeqCst)
    }
}

/// `state-dump`'s options.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DumpArgs {
    /// `--save FILE.d2s`: the character (as `play --save`); none: `play`'s
    /// default character.
    pub save: Option<PathBuf>,
    /// `--seed N`: the fixed game seed (as `play --seed`).
    pub seed: Option<u32>,
    /// `--difficulty normal|nightmare|hell|0-2`.
    pub difficulty: u8,
    /// `--ticks T`: server ticks to run.
    pub ticks: u32,
    /// `--every n`: snapshot the frames that are multiples of n (default 1).
    pub every: u32,
    /// `--out FILE`.
    pub out: PathBuf,
    /// `--game-dir DIR`, else `$D2_GAME_DIR`.
    pub game_dir: Option<PathBuf>,
    /// `--date YYYY-MM-DD` for the header, else today (UTC).
    pub date: Option<String>,
    /// `--poke "<f> <directive> <args>..."` (repeatable): absolute frames.
    pub pokes: Vec<Entry>,
    /// `--input SCRIPT`: the shared frame-anchored input (scenario-diff.md §2 r4).
    pub input: Option<Vec<input_script::Step>>,
    /// `--send "<f> <Name|hex> ..."` (repeatable): scripted C→S messages.
    pub sends: Vec<SendEntry>,
    /// `--no-own-c2s <id>[,<id>]`: C→S ids the bridge's own sends drop.
    pub no_own_c2s: Vec<u8>,
    /// `--packets FILE`: also record the packets (`specs/tools/packets-trace.md`).
    pub packets: Option<PathBuf>,
    /// `--rng FILE`: also record every RNG draw ([`super::rng_dump`];
    /// needs the `rng-trace` feature).
    pub rng: Option<PathBuf>,
    /// `--save-out FILE`: install the character writer of `play` with this
    /// path, so a `--send "<f> hex 69"` (Save and Exit) writes the `.d2s`
    /// there (the `save` channel of `specs/tools/scenario-diff.md`).
    pub save_out: Option<PathBuf>,
}

/// Parses the options after `state-dump`.
pub fn parse_args(args: &[String]) -> Result<DumpArgs> {
    let mut a = DumpArgs {
        save: None,
        seed: None,
        difficulty: 0,
        ticks: 0,
        every: 1,
        out: PathBuf::new(),
        game_dir: None,
        date: None,
        pokes: Vec::new(),
        input: None,
        sends: Vec::new(),
        no_own_c2s: Vec::new(),
        packets: None,
        rng: None,
        save_out: None,
    };
    let (mut ticks, mut out) = (None, None);
    let mut it = args.iter();
    while let Some(flag) = it.next() {
        let mut value = || it.next().with_context(|| format!("{flag} needs a value"));
        match flag.as_str() {
            "--save" => a.save = Some(PathBuf::from(value()?)),
            "--seed" => a.seed = Some(value()?.parse().context("--seed")?),
            "--difficulty" => {
                let v = value()?;
                a.difficulty = single_player::parse_difficulty(v).with_context(|| {
                    format!("--difficulty {v}: use normal, nightmare, hell or 0-2")
                })?;
            }
            "--ticks" => ticks = Some(value()?.parse::<u32>().context("--ticks")?),
            "--every" => a.every = value()?.parse().context("--every")?,
            "--out" => out = Some(PathBuf::from(value()?)),
            "--game-dir" => a.game_dir = Some(PathBuf::from(value()?)),
            "--packets" => a.packets = Some(PathBuf::from(value()?)),
            "--rng" => a.rng = Some(PathBuf::from(value()?)),
            "--save-out" => a.save_out = Some(PathBuf::from(value()?)),
            "--poke" => a
                .pokes
                .push(pokes::parse_poke_arg(value()?).map_err(anyhow::Error::msg)?),
            "--send" => a
                .sends
                .push(sends::parse_send_arg(value()?).map_err(anyhow::Error::msg)?),
            "--no-own-c2s" => {
                for part in value()?.split(',') {
                    let t = part.trim();
                    let id = match t.strip_prefix("0x") {
                        Some(h) => u8::from_str_radix(h, 16),
                        None => t.parse::<u8>(),
                    }
                    .with_context(|| format!("--no-own-c2s {t}: a message id (0-255, 0x hex)"))?;
                    if !a.no_own_c2s.contains(&id) {
                        a.no_own_c2s.push(id);
                    }
                }
            }
            "--input" => {
                let steps = input_script::parse(value()?)
                    .and_then(|s| Headless::new(s.clone()).map(|_| s))
                    .map_err(|e| anyhow::anyhow!("--input: {e}"))?;
                a.input = Some(steps);
            }
            "--date" => {
                let v = value()?.clone();
                if !is_date(&v) {
                    bail!("--date {v}: use YYYY-MM-DD");
                }
                a.date = Some(v);
            }
            other => bail!("state-dump: unknown option {other}"),
        }
    }
    a.ticks = ticks.context("state-dump needs --ticks T")?;
    a.out = out.context("state-dump needs --out FILE")?;
    if a.every == 0 {
        bail!("--every must be at least 1");
    }
    Ok(a)
}

fn is_date(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 10
        && b[4] == b'-'
        && b[7] == b'-'
        && b.iter()
            .enumerate()
            .all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit())
}

/// Today's date (UTC) as `YYYY-MM-DD`, from the system clock.
pub fn today() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    civil_date((secs / 86_400) as i64)
}

/// The proleptic Gregorian date of `days` since 1970-01-01.
fn civil_date(days: i64) -> String {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}

/// The game the dump runs: what `play` resolves from the same options.
pub struct DumpGame {
    pub data: GameData,
    pub character: Character,
    /// The game seed (`single_player::game_seed`).
    pub seed: u32,
    pub hardcore: bool,
    /// The 0x67 flags; `None`: the character's own.
    pub start_flags: Option<u32>,
    /// The `--poke` entries, in command-line order.
    pub pokes: Vec<Entry>,
    /// The `--input` steps.
    pub input: Option<Vec<input_script::Step>>,
    /// The `--send` entries, in command-line order.
    pub sends: Vec<SendEntry>,
    /// `--no-own-c2s` ids.
    pub no_own_c2s: Vec<u8>,
    /// `--packets FILE` ([`super::packet_dump`]).
    pub packets: Option<PathBuf>,
    /// `--rng FILE` ([`super::rng_dump`]).
    pub rng: Option<PathBuf>,
    /// `--save-out FILE`: where the server's character writer puts the
    /// `.d2s` (`play`'s [`super::save::FileStore`]).
    pub save_out: Option<PathBuf>,
}

impl DumpGame {
    /// Resolves `args` the way `d2-client play` resolves its command line
    /// (`main::play_once`): the character of `--save` (or the default),
    /// the difficulty, the seed, the hardcore bit of the save.
    pub fn resolve(args: &DumpArgs, data: GameData, game_dir: Option<&Path>) -> Result<Self> {
        let start = play_start::resolve(
            &CliStart {
                save: args.save.clone(),
                new: None,
                save_dir: None,
                difficulty: args.difficulty,
                hardcore: false,
            },
            &data,
            game_dir,
            None,
            None,
        )?;
        let seed = single_player::game_seed(&start.character, args.seed);
        // `play::run`: a loaded save's own status bit makes it hardcore.
        let hardcore = start.hardcore
            || super::save::base_save(&start.character).header.status
                & d2_formats::d2s::status::HARDCORE
                != 0;
        Ok(Self {
            data,
            character: start.character,
            seed,
            hardcore,
            start_flags: start.start_flags,
            pokes: args.pokes.clone(),
            input: args.input.clone(),
            sends: args.sends.clone(),
            no_own_c2s: args.no_own_c2s.clone(),
            packets: args.packets.clone(),
            rng: args.rng.clone(),
            save_out: args.save_out.clone(),
        })
    }
}

/// What a dump wrote.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DumpReport {
    /// Server ticks run.
    pub ticks: u32,
    /// Snapshots written.
    pub snaps: u64,
}

/// The header fields of the run (all but `fields` / `gaps`, which come
/// from the game).
pub struct RunInfo {
    pub tool: String,
    pub date: String,
    pub command: String,
    pub save: Option<String>,
}

/// What the dump knows it does not match, beyond the unit fields: the
/// client side is the bridge alone.
pub const RUN_GAPS: [&str; 1] = [
    "client: headless bridge (no UI or visibility art); the only C->S messages are 0x67, \
     the model's own answers (0x6B, 0x5F, 0x28's 0x2F / 0x30 and its dialog branch's 0x31 from the headless original UI), the --send messages and the --input clicks (world-click dispatcher \
     with the play preview's hover pick, the local player at the play preview's walk \
     prediction, held repeat once per server frame; keys: belt 1-4, run lock, weapon swap, \
     speech only), so a run \
     where the 1.14d client sends anything else differs from the first such tick",
];

/// Runs `game` for `ticks` server ticks and writes the `state-1` lines to
/// `out` (one snapshot per tick whose frame is a multiple of `every`).
pub fn dump<W: Write>(
    game: DumpGame,
    ticks: u32,
    every: u32,
    info: &RunInfo,
    out: &mut W,
) -> Result<DumpReport> {
    let every = every.max(1);
    let ms = Arc::new(AtomicU32::new(START_MS));
    let client_data = ClientData::of(&game.data)?;
    let speeds = single_player::walk_speeds(&game.data, &game.character)?;
    // `play`'s character writer (`play::run`), for a `--send "<f> hex 69"`.
    let store = match &game.save_out {
        Some(path) => {
            let GameData::Live(live) = &game.data;
            let mut base = super::save::base_save(&game.character);
            if game.hardcore {
                base.header.status |= d2_formats::d2s::status::HARDCORE;
            }
            Some(super::save::FileStore {
                path: path.clone(),
                base,
                tables: Arc::new(live.save.clone()),
                appearance: Some(Arc::new(
                    super::save::appearance_tables(&live.tables.fixed)
                        .map_err(anyhow::Error::msg)?,
                )),
            })
        }
        None => None,
    };
    let mut rng = match &game.rng {
        Some(p) => Some(super::rng_dump::RngDump::create(p, info, game.seed)?),
        None => None,
    };
    let (mut link, _started) = super::rng_dump::start_with(
        game.data,
        game.seed,
        game.character.clone(),
        StepClock(ms.clone()),
        rng.is_some(),
    )?;
    if game.hardcore {
        link.with(|l| l.host_mut().game.events.action.hooks().x.hardcore = true)?;
    }
    let saving = store.is_some();
    if let Some(store) = store {
        link.with(move |l| l.host_mut().game.set_storage(Box::new(store)))?;
    }
    let mut packets = match &game.packets {
        Some(p) => Some(super::packet_dump::PacketDump::create(
            &mut link, p, info, game.seed,
        )?),
        None => None,
    };
    let (mut fields, mut gaps) = link.with(|l| state::coverage_world(&l.host().game.events))?;
    // `q`: the players' quest records, kept by the app's rest (StateSource below)
    fields.extend(state::HOST_FIELDS.iter().map(|k| (*k).to_owned()));
    gaps.extend(RUN_GAPS.iter().map(|g| (*g).to_owned()));
    let header = state::Header {
        side: "d2rs".into(),
        tool: info.tool.clone(),
        date: info.date.clone(),
        command: info.command.clone(),
        fields,
        gaps,
        save: info.save.clone(),
        seed: Some(game.seed),
    };
    writeln!(out, "{}", header.to_json_line())?;

    // `poke.md` §5 rule 5: the pokes due after a tick run at the tick end
    // (the 1.14d hook's point), after that frame's snapshot; a `goto` steps
    // there on every later frame until it ends.
    let (tick_end_entries, rest) = pokes::split_tick_end(game.pokes);
    let tick_end = Arc::new(Mutex::new(TickEndOut::default()));
    if !tick_end_entries.is_empty() {
        tick_end.lock().unwrap_or_else(|e| e.into_inner()).pending = tick_end_entries.len();
        let shared = tick_end.clone();
        let mut entries = tick_end_entries;
        link.with(move |l| {
            l.set_tick_end(Box::new(move |h| {
                let frame = h.game.game.frame;
                let (due, later): (Vec<Entry>, Vec<Entry>) = std::mem::take(&mut entries)
                    .into_iter()
                    .partition(|e| pokes::due_after(e.when, None).is_some_and(|f| frame >= f));
                entries = later;
                let mut out = shared.lock().unwrap_or_else(|e| e.into_inner());
                if due.is_empty() && out.walking.is_empty() {
                    return;
                }
                out.snapshot = Some((frame, snapshot_host(h)));
                // `goto` walks step first, then the pokes due now (`poke.md` §6).
                let due = std::mem::take(&mut out.walking)
                    .into_iter()
                    .map(|(e, w)| (e, Some(w)))
                    .chain(due.into_iter().map(|e| (e, None)));
                for (i, (e, walk)) in due.enumerate() {
                    let When::Frame(f) = e.when else { continue };
                    let line = match (&e.op, walk) {
                        (PokeOp::Directive(Directive::Goto(t)), w) => {
                            let (r, w) = pokes::goto_now(&mut h.game, *t, w.unwrap_or_default());
                            if r == PokeResult::Pending {
                                out.walking.push((e, w));
                                continue;
                            }
                            // a walk's record carries the frame of its last step
                            pokes::record_line_steps(frame + 1, i, &e.op, &r, Some(w.steps))
                        }
                        _ => {
                            let r = pokes::apply_on_host(h, &e.op);
                            pokes::record_line(f, i, &e.op, &r)
                        }
                    };
                    out.lines.push(line);
                }
                pokes::queue_sent_now(h);
                out.pending = entries.len() + out.walking.len();
            }))
        })?;
    }
    let link = PredictLink::new(link);
    let tap = link.tap();
    let mut bridge = Bridge::new(link)?;
    client_data.install(&mut bridge);
    let mut request = single_player::create_request_for(&game.character);
    if let Some(f) = game.start_flags {
        request.flags = f;
    }
    bridge.send(&request)?;
    bridge.set_drop_own(game.no_own_c2s.clone());
    let mut ui = DialogUi::new()?;
    let (mut ran, mut snaps, mut idle) = (0u32, 0u64, 0u32);
    let mut first = true;
    // The frame the last tick ran (0: none yet) and the pokes still to run.
    let mut last_frame = 0i32;
    let mut pending = rest;
    // The client part follows the server's walk and point as the play
    // preview does, with or without an input script (REC-1385).
    let mut input = Some(
        Headless::new(game.input.unwrap_or_default())
            .map_err(anyhow::Error::msg)?
            .with_prediction(tap, speeds),
    );
    let mut input_notes = Vec::new();
    let mut to_send = game.sends;
    let mut send_notes = Vec::new();
    let (mut joined, mut notes_left) = (false, None::<u32>);
    while ran < ticks {
        run_due_pokes(&mut bridge, &mut pending, last_frame, out)?;
        if let Some(h) = input.as_mut() {
            for l in h.apply(&mut bridge, last_frame)? {
                eprintln!("input: {l}");
                input_notes.push(format!("input: {l}"));
            }
        }
        run_due_sends(&mut bridge, &mut to_send, last_frame, out, &mut send_notes)?;
        if !std::mem::replace(&mut first, false) {
            ms.fetch_add(STEP_MS, Ordering::SeqCst);
        }
        bridge.set_now(ms.load(Ordering::SeqCst));
        let t0 = super::perf::enabled().then(std::time::Instant::now);
        let report = bridge.frame()?;
        if let Some(t0) = t0 {
            super::perf::record_bridge_frame(t0, report.ticked);
        }
        let outputs = bridge.take_outputs();
        ui.deliver(&mut bridge, &outputs)?;
        for m in bridge.take_dropped() {
            let hex: Vec<String> = m.iter().map(|b| format!("{b:02x}")).collect();
            let line = format!(
                "own c2s dropped: frame {}: {}",
                last_frame + 1,
                hex.join(" ")
            );
            eprintln!("{line}");
            send_notes.push(line);
        }
        // Save and Exit (C→S 0x69) took the client out of the game: the
        // server wrote the file in its leave (`flows/save-exit.md` §2 r2).
        if saving {
            let in_game = bridge.world().in_game;
            if in_game {
                joined = true;
            } else if joined {
                notes_left = Some(ran);
                break;
            }
        }
        if let Some(h) = input.as_mut() {
            for l in h.after_frame(&mut bridge, report.ticked)? {
                eprintln!("input: {l}");
                input_notes.push(format!("input: {l}"));
            }
            h.sync_local(&mut bridge);
        }
        if let Some(p) = packets.as_mut() {
            p.drain()?;
        }
        if let Some(r) = rng.as_mut() {
            r.drain(&mut bridge)?;
        }
        if !report.ticked {
            idle += 1;
            if idle > MAX_IDLE_STEPS {
                bail!("no server tick in {MAX_IDLE_STEPS} steps (after {ran} ticks)");
            }
            continue;
        }
        idle = 0;
        ran += 1;
        // The tick-end pokes' records, and the snapshot taken before them.
        let held = {
            let mut t = tick_end.lock().unwrap_or_else(|e| e.into_inner());
            for line in t.lines.drain(..) {
                eprintln!("poke: at the tick end: {line}");
                writeln!(out, "{line}")?;
            }
            t.snapshot.take()
        };
        let s = match held {
            Some((_, s)) => s,
            None => bridge.state_snapshot()?,
        };
        last_frame = s.frame;
        if s.frame.rem_euclid(every as i32) == 0 {
            writeln!(out, "{}", s.to_json_line())?;
            snaps += 1;
        }
    }
    let mut notes = vec![format!(
        "{ran} server ticks, clock {STEP_MS} ms per step from {START_MS} ms, every {every}"
    )];
    if let Some(n) = notes_left {
        notes.push(format!("left the game after {n} ticks (Save and Exit)"));
    }
    notes.extend(input_notes);
    notes.extend(send_notes);
    notes.extend(ui.notes);
    for e in &to_send {
        eprintln!(
            "send: not reached in {ran} ticks: frame {} {}",
            e.frame,
            e.text()
        );
        notes.push(format!("send not reached: frame {} {}", e.frame, e.text()));
    }
    if let Some(n) = input.as_ref().map(Headless::pending).filter(|&n| n > 0) {
        eprintln!("input: {n} step(s) not reached in {ran} ticks");
        notes.push(format!("input: {n} step(s) not reached"));
    }
    let (unreached, tick_end_walks) = tick_end.lock().map_or((0, Vec::new()), |mut t| {
        let walks = std::mem::take(&mut t.walking);
        (t.pending.saturating_sub(walks.len()), walks)
    });
    let walking = tick_end_walks;
    if unreached > 0 {
        eprintln!("poke: {unreached} tick-end poke(s) not reached in {ran} ticks");
        notes.push(format!("poke not reached: {unreached} tick-end poke(s)"));
    }
    for e in &pending {
        eprintln!("poke: not reached in {ran} ticks: {:?} {}", e.when, e.op);
        notes.push(format!("poke not reached: {:?} {}", e.when, e.op));
    }
    // A `goto` still walking at the end is `failed` (`poke.md` §6 r4).
    for (i, (e, w)) in walking.iter().enumerate() {
        let f = last_frame + 1;
        let r = d2_sim::poke::PokeResult::Failed;
        let line = pokes::record_line_steps(f, i, &e.op, &r, Some(w.steps));
        eprintln!("poke: goto not finished in {ran} ticks: {line}");
        notes.push(format!("goto not finished: {}", e.op));
        writeln!(out, "{line}")?;
    }
    if let Some(p) = packets {
        p.finish(&notes)?;
    }
    if let Some(r) = rng {
        r.finish(&notes)?;
    }
    writeln!(out, "{}", state::footer_line(snaps, &notes))?;
    out.flush()?;
    Ok(DumpReport { ticks: ran, snaps })
}

/// Runs every pending poke due after `last_frame` (absolute `f` with
/// f − 1 ≤ `last_frame`, `poke.md` §2 r6) through the bridge, in order,
/// writing a `poke` line each and printing it to stderr. Only the pokes
/// due before the first tick get here; the others run at the tick end
/// ([`pokes::split_tick_end`]).
fn run_due_pokes<W: Write>(
    bridge: &mut Bridge<DumpLink>,
    pending: &mut Vec<Entry>,
    last_frame: i32,
    out: &mut W,
) -> Result<()> {
    let (due, later): (Vec<Entry>, Vec<Entry>) = std::mem::take(pending)
        .into_iter()
        .partition(|e| pokes::due_after(e.when, None).is_some_and(|f| last_frame >= f));
    *pending = later;
    for (i, e) in due.iter().enumerate() {
        // state-dump takes absolute --poke frames only (never a Tick entry)
        let When::Frame(f) = e.when else { continue };
        let r = bridge.poke(&e.op)?;
        let line = pokes::record_line(f, i, &e.op, &r);
        eprintln!("poke: before frame {f}: {}: {line}", e.op);
        writeln!(out, "{line}")?;
    }
    Ok(())
}

/// Injects every pending `--send` due after `last_frame` (f − 1 ≤
/// `last_frame`) through the bridge, in order, writing a `send` line each,
/// printing it to stderr and keeping it for the footer notes.
fn run_due_sends<W: Write>(
    bridge: &mut Bridge<DumpLink>,
    pending: &mut Vec<SendEntry>,
    last_frame: i32,
    out: &mut W,
    notes: &mut Vec<String>,
) -> Result<()> {
    let (due, later): (Vec<SendEntry>, Vec<SendEntry>) = std::mem::take(pending)
        .into_iter()
        .partition(|e| last_frame >= e.frame - 1);
    *pending = later;
    for (i, e) in due.iter().enumerate() {
        let r = bridge.inject(&e.msg)?;
        let line = sends::record_line(e.frame, i, &e.text(), &r);
        eprintln!("send: before frame {}: {line}", e.frame);
        notes.push(format!("send: {line}"));
        writeln!(out, "{line}")?;
    }
    Ok(())
}

/// The server thread's snapshot, taken on the server thread between two
/// frames (`state-snapshot.md` §3 r1), with each player's `q`: the quest
/// record of the game's difficulty from the rest's per-player quests
/// (`AppRest::quests`, `world/quests.md` §1.4). Reads only.
impl<C: Clock + Send + 'static> StateSource for ThreadLink<Link<C>> {
    type Error = super::server_thread::ThreadStopped;
    fn state_snapshot(&mut self) -> Result<state::StateSnapshot, Self::Error> {
        self.with(|l| snapshot_host(l.host()))
    }
}

/// What the tick-end hook hands back to the dump loop: the poke records
/// written at the tick end, the snapshot of that frame taken before them
/// (1.14d `record_state.py` snapshots at the same hook before its pokes),
/// the `goto` walks still stepping there and how many tick-end pokes
/// (walks included) are still to run.
#[derive(Default)]
struct TickEndOut {
    lines: Vec<String>,
    /// The `goto` walks still stepping at the tick end (`poke.md` §6).
    walking: Vec<(Entry, GotoWalk)>,
    snapshot: Option<(i32, state::StateSnapshot)>,
    pending: usize,
}

/// The snapshot of the host's game (the [`StateSource`] above, and the
/// tick-end hook's).
fn snapshot_host<C: Clock>(h: &pokes::ServerHost<C>) -> state::StateSnapshot {
    let sim = &h.game;
    let mut s = state::snapshot_world(&sim.game, &sim.events);
    if let Some(inv) = sim.world.inventory.as_ref() {
        overlay_item_places(&mut s, &inv.state);
        overlay_item_owners(
            &mut s,
            &inv.state,
            &sim.game.lists,
            &sim.events.action.sys.units,
        );
    }
    let d = usize::from(state::difficulty_world(&sim.events)).min(2);
    for (id, q) in &sim.world.rest.quests {
        if let Some(e) = sim.game.lists.unit(*id) {
            s.set_quests(e.guid, state::quest_words(&q.flags[d]));
        }
    }
    // PROVISIONAL (state-snapshot.md `q`, REC-1960): a player corpse
    // (mode 17) holds an all-zero quest record on 1.14d (recorded
    // `gen-boss-708`, frame 83: `q: []`); d2rs keeps no record for it.
    // Settled by reading the corpse's player-data init in 1.14d.
    for u in &mut s.units {
        if u.ut == d2_sim::units::lists::UnitType::Player as u8 && u.m == Some(17) && u.q.is_none()
        {
            u.q = Some(Vec::new());
        }
    }
    s
}

/// An item in an inventory has a static path on 1.14d whose x, y are its
/// place (the cell of a page, the belt slot, the body location) and whose
/// direction is 0 (`items-load-mixed` against 1.14d, frame 2; `state-
/// snapshot.md` §2). d2rs keeps the place in the inventory model, not in
/// a path record, so the export reads it there, also for an item that just
/// left the ground (its ground path is stale; REC-1402).
fn overlay_item_places(snap: &mut state::StateSnapshot, inv: &d2_sim::wiring::inventory::InvState) {
    use d2_sim::items::moves::mode;
    for u in snap.units.iter_mut().filter(|u| u.ut == 4) {
        // A monster's equipment (owner none, equipped) has its body
        // location in its own static path, which the snapshot already
        // carries; a store's stock (owner none, stored) is placed in
        // the store's page like a player's item.
        let placed = inv.items.values().find(|d| {
            d.guid == u.g
                && match d.mode {
                    mode::STORED => true,
                    mode::EQUIPPED | mode::BELT => {
                        d.owner_guid != d2_sim::items::inventory::NO_GUID
                    }
                    _ => false,
                }
        });
        if let Some(d) = placed {
            u.x = u32::try_from(d.x).ok();
            u.y = u32::try_from(d.y).ok();
            u.d = Some(0);
            // The static path's room is 0: no `lv` (state-snapshot.md §6 r1).
            u.lv = None;
        }
    }
}

/// `own` of an item (`state-snapshot.md` §2): the holder of the inventory
/// the item is linked into (item data +0x5C -> inventory +0x08 owner
/// unit); absent on the ground (no inventory) or with a 0 GUID. d2rs
/// keeps the link in the inventory model.
///
/// A store item (vendor flag, unit +0xC8 bit 2: `world/vendors.md` §3.1
/// step 5) reads absent: recorded `items-vendor-akara-buy`, the 41 store
/// items of Akara at frames 20-40 have no `own` on 1.14d while the
/// bought copy in the player's inventory has the player's GUID. The
/// store's inventory has no owner unit (`vendors.md` §4 rule 3, "new NPC
/// inventory").
fn overlay_item_owners(
    snap: &mut state::StateSnapshot,
    inv: &d2_sim::wiring::inventory::InvState,
    lists: &d2_sim::units::UnitLists,
    units: &d2_sim::units::record::Units,
) {
    for u in snap.units.iter_mut().filter(|u| u.ut == 4) {
        let found = inv.items.iter().find(|(_, d)| d.guid == u.g);
        let vendor = found.is_some_and(|(id, _)| {
            units
                .get(*id)
                .is_some_and(|r| r.flags2 & d2_sim::items::moves::deferred::VENDOR_ITEM != 0)
        });
        let holder = found
            .and_then(|(_, d)| d.inv)
            .and_then(|o| lists.unit(o))
            .map(|e| e.guid);
        // A monster's equipment has no inventory model: the snapshot's
        // own holder (sim `owner`) stays.
        if vendor {
            u.own = None;
        } else if holder.is_some() {
            u.own = holder.filter(|&g| g != 0);
        }
    }
}

/// The bridge-level client data `play` installs (`play::add_live_client`):
/// what the model's own answers can depend on.
struct ClientData {
    drlg: crate::bridge::drlg::DrlgSource,
    levels: Vec<crate::bridge::world::LevelRow>,
    objects: Vec<crate::bridge::objects::ObjClientRow>,
    skills: Vec<crate::bridge::world::SkillRow>,
    class_skills: Vec<[u16; 10]>,
    skill_tables: d2_sim::skills::SkillTables,
    units: crate::bridge::world::UnitRows,
    player_anims: super::anim_names::ClientPlayerAnims,
    item_tables: Arc<d2_sim::items::ItemTables>,
}

impl ClientData {
    fn of(data: &GameData) -> Result<Self> {
        let GameData::Live(d) = data;
        let archives = d.archives.as_ref();
        Ok(Self {
            drlg: single_player::client_drlg_source(data),
            levels: single_player::client_level_rows(data),
            objects: single_player::client_object_rows(data),
            skills: single_player::client_skill_rows(archives)?,
            class_skills: single_player::client_class_skills(archives)?,
            skill_tables: single_player::client_skill_tables(archives)?,
            units: {
                let mut u = single_player::client_unit_rows(archives)?;
                single_player::client_monster_anims(archives, &mut u)?;
                u
            },
            player_anims: single_player::client_player_anims(data)?,
            // The item type test of the use state (`bridge::use_state`).
            item_tables: Arc::new(d.tables.item_tables().map_err(|e| anyhow::anyhow!("{e}"))?),
        })
    }

    fn install<L: crate::bridge::link::ServerLink>(self, b: &mut Bridge<L>) {
        b.set_drlg_source(Some(self.drlg));
        b.set_levels(self.levels);
        b.set_object_rows(self.objects);
        b.set_skill_rows(self.skills);
        b.set_class_skills(self.class_skills);
        b.set_skill_tables(Arc::new(self.skill_tables));
        b.set_unit_rows(self.units);
        b.set_player_anims(Arc::new(self.player_anims));
        b.set_item_tables(Arc::new(super::items::TableDecoder(self.item_tables)));
        b.set_high_light_quality(true);
    }
}

/// `d2-client state-dump ...`: loads the user's files (`--game-dir` or
/// `$D2_GAME_DIR`), resolves the game as `play`, runs the dump into
/// `--out`. `command`: the argv, joined.
pub fn run(args: &DumpArgs, command: &str) -> Result<DumpReport> {
    let dir = args
        .game_dir
        .clone()
        .or_else(|| std::env::var_os("D2_GAME_DIR").map(PathBuf::from));
    let data = GameData::select(dir.as_deref())?;
    let game = DumpGame::resolve(args, data, dir.as_deref())?;
    let info = RunInfo {
        tool: format!("d2-client state-dump {}", env!("CARGO_PKG_VERSION")),
        date: args.date.clone().unwrap_or_else(today),
        command: command.to_owned(),
        save: args.save.as_ref().map(|p| p.display().to_string()),
    };
    if let Some(parent) = args.out.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating {}", parent.display()))?;
    }
    let file = std::fs::File::create(&args.out)
        .with_context(|| format!("creating {}", args.out.display()))?;
    let mut w = std::io::BufWriter::new(file);
    dump(game, args.ticks, args.every, &info, &mut w)
}

/// The UI layer's part of the client the dump runs: the original UI
/// (`ui/original.rs`, headless: no panels drawn, nothing shown) takes the
/// bridge's outputs in list order, as `play`'s output dispatcher does
/// (`world_view::present::deliver`, `client/bridge.md` §10 rules 4-5), so
/// the answer of 0x28's dialog branch (`client/msg-ui.md` §16 r4.3, case B2:
/// C→S 0x31 right after the model's 0x2F) is sent as 1.14d's client sends
/// it. Audio and effects outputs have no consumer here.
struct DialogUi {
    ui: crate::ui::original::OriginalUi,
    /// The UI errors met, once each (footer notes).
    notes: Vec<String>,
}

impl DialogUi {
    fn new() -> Result<Self> {
        // `expansion_installed` (`0x00408F20`, d2exp.mpq present): the
        // checks run against the 1.14d LoD install (PROVISIONAL REC-1686:
        // `play` reads it from the archives, `app/ui.rs` `UiParts::live`).
        let config = crate::ui::original::UiConfig {
            screen: crate::ui::layout::Screen::play(),
            expansion_installed: true,
        };
        let ui = crate::ui::original::OriginalUi::new(config, None)
            .map_err(|e| anyhow::anyhow!("original UI: {e:?}"))?;
        Ok(Self {
            ui,
            notes: Vec::new(),
        })
    }

    fn deliver(&mut self, bridge: &mut Bridge<DumpLink>, outputs: &[Output]) -> Result<()> {
        for o in outputs.iter().filter(|o| o.consumer() == Consumer::Ui) {
            if let Err(e) = self.ui.apply_output(o, bridge.world()) {
                let note = format!("ui: {e}");
                if !self.notes.contains(&note) {
                    eprintln!("{note}");
                    self.notes.push(note);
                }
            }
            self.ui.take_sounds();
            self.ui.take_skipped();
            if let Some((d, case)) = self.ui.take_dialog_answer() {
                bridge.npc_dialog_branch(&d, case)?;
            }
        }
        Ok(())
    }
}

/// The link the dump's bridge runs on: the server thread inside the
/// walk-recording [`PredictLink`] (`--input`'s click position).
pub type DumpLink = PredictLink<ThreadLink<Link<StepClock>>>;

#[cfg(test)]
mod tests {
    use super::*;

    fn args(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn options_parse() {
        let a = parse_args(&args(&[
            "--save",
            "S.d2s",
            "--seed",
            "1234",
            "--ticks",
            "5",
            "--every",
            "2",
            "--out",
            "o",
            "--difficulty",
            "hell",
            "--game-dir",
            "g",
            "--date",
            "2026-10-09",
            "--packets",
            "p.jsonl",
            "--save-out",
            "out.d2s",
            "--no-own-c2s",
            "0x5f,3,0x5f",
            "--poke",
            "4 spawn 19 @x+3 @y+3 normal",
            "--poke",
            "4 seed-unit @1:19 0x12345678 666",
            "--input",
            "frame 10; click 600 300",
            "--send",
            "7 InteractWithEntity type=1 id=@1:148",
            "--send",
            "7 hex 2f 00 00 00 00 09 00 00 00",
        ]))
        .unwrap();
        let pokes = a.pokes.clone();
        let sends = a.sends.clone();
        assert_eq!(sends.len(), 2);
        assert_eq!(sends[0].frame, 7);
        assert_eq!(sends[0].text(), "InteractWithEntity type=1 id=@1:148");
        assert_eq!(sends[1].text(), "hex 2f 00 00 00 00 09 00 00 00");
        assert_eq!(pokes.len(), 2);
        assert_eq!(pokes[0].when, When::Frame(4));
        assert_eq!(pokes[1].op.to_string(), "seed-unit @1:19 305419896 666");
        assert_eq!(
            a,
            DumpArgs {
                save: Some("S.d2s".into()),
                seed: Some(1234),
                difficulty: 2,
                ticks: 5,
                every: 2,
                out: "o".into(),
                game_dir: Some("g".into()),
                date: Some("2026-10-09".into()),
                pokes,
                input: Some(input_script::parse("frame 10; click 600 300").unwrap()),
                sends,
                no_own_c2s: vec![0x5f, 0x03],
                packets: Some("p.jsonl".into()),
                rng: None,
                save_out: Some("out.d2s".into()),
            }
        );
        assert!(parse_args(&args(&[
            "--ticks",
            "1",
            "--out",
            "o",
            "--poke",
            "0 time 1 0"
        ]))
        .is_err());
        assert!(parse_args(&args(&["--ticks", "1", "--out", "o", "--poke", "3 nope"])).is_err());
        for bad in ["0 Walk x=1 y=2", "3 Walk x=1", "3 Nope a=1", "3 hex 1"] {
            assert!(
                parse_args(&args(&["--ticks", "1", "--out", "o", "--send", bad])).is_err(),
                "{bad}"
            );
        }
        assert!(parse_args(&args(&["--out", "o"])).is_err(), "no --ticks");
        assert!(parse_args(&args(&["--ticks", "1"])).is_err(), "no --out");
        assert!(parse_args(&args(&["--ticks", "1", "--out", "o", "--every", "0"])).is_err());
        assert!(parse_args(&args(&["--ticks", "1", "--out", "o", "--date", "9.10.26"])).is_err());
        assert!(parse_args(&args(&["--ticks", "1", "--out", "o", "--frames", "3"])).is_err());
        // the shared input form only (scenario-diff.md §3 r8)
        for bad in ["click 1 2", "frame 2; wait 3", "frame 2; key i", "frame 0"] {
            assert!(
                parse_args(&args(&["--ticks", "1", "--out", "o", "--input", bad])).is_err(),
                "{bad}"
            );
        }
    }

    // Covers: specs/tools/poke.md §2 r6
    #[test]
    fn poke_record_lines() {
        use d2_sim::poke::PokeResult;
        let op = pokes::parse_poke_arg("4 spawn 19 @x+3 @y+3 normal")
            .unwrap()
            .op;
        assert_eq!(
            pokes::record_line(4, 0, &op, &PokeResult::Ok(Some(8))),
            r#"{"k":"poke","f":4,"frame":3,"i":0,"d":"spawn","r":"ok","guid":8,"src":"spawn 19 @x+3 @y+3 normal"}"#
        );
        let op = pokes::parse_poke_arg("5 seed-unit @1:19 1 2").unwrap().op;
        assert_eq!(
            pokes::record_line(5, 1, &op, &PokeResult::Unresolved("@1:19".into())),
            r#"{"k":"poke","f":5,"frame":4,"i":1,"d":"seed-unit","r":"unresolved","note":"@1:19","src":"seed-unit @1:19 1 2"}"#
        );
        let op = pokes::parse_poke_arg("6 msg 0x01 @x+2 @y").unwrap().op;
        assert_eq!(
            pokes::record_line(
                6,
                0,
                &op,
                &PokeResult::FailedWith("duplicate filter".into())
            ),
            r#"{"k":"poke","f":6,"frame":5,"i":0,"d":"msg","r":"failed","note":"duplicate filter","src":"msg 1 @x+2 @y"}"#
        );
    }

    #[test]
    fn civil_dates() {
        assert_eq!(civil_date(0), "1970-01-01");
        assert_eq!(civil_date(11_016), "2000-02-29");
        assert_eq!(civil_date(20_735), "2026-10-09");
        assert!(is_date(&today()));
    }
}
