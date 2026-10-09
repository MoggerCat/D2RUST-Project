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
//! Output: header, snaps, footer (§1). Two runs with the same arguments
//! write the same bytes but for the header's `date`. Not game logic: the
//! clock and the date are this binary's (CLAUDE.md rule 6 binds
//! `d2-sim`, which only reads here).

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use anyhow::{bail, Context, Result};
use d2_server::seams::Clock;
use d2_sim::debug::state;

use super::play_start::{self, CliStart};
use super::server_thread::ThreadLink;
use super::single_player::{self, Character, GameData, Link};
use crate::bridge::Bridge;

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
    "client: headless bridge (no UI, input, prediction or visibility art); the only C->S \
     messages are 0x67 and the model's own answers (0x6B, 0x5F), so a run where the 1.14d \
     client sends anything else differs from the first such tick",
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
    let (mut link, _started) = single_player::start_with(
        game.data,
        game.seed,
        game.character.clone(),
        StepClock(ms.clone()),
    )?;
    if game.hardcore {
        link.with(|l| l.host_mut().game.events.action.hooks().x.hardcore = true)?;
    }
    let (fields, mut gaps) = link.with(|l| state::coverage_world(&l.host().game.events))?;
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

    let mut bridge = Bridge::new(link)?;
    client_data.install(&mut bridge);
    let mut request = single_player::create_request_for(&game.character);
    if let Some(f) = game.start_flags {
        request.flags = f;
    }
    bridge.send(&request)?;
    let (mut ran, mut snaps, mut idle) = (0u32, 0u64, 0u32);
    let mut first = true;
    while ran < ticks {
        if !std::mem::replace(&mut first, false) {
            ms.fetch_add(STEP_MS, Ordering::SeqCst);
        }
        bridge.set_now(ms.load(Ordering::SeqCst));
        let report = bridge.frame()?;
        bridge.take_outputs();
        if !report.ticked {
            idle += 1;
            if idle > MAX_IDLE_STEPS {
                bail!("no server tick in {MAX_IDLE_STEPS} steps (after {ran} ticks)");
            }
            continue;
        }
        idle = 0;
        ran += 1;
        let snap = bridge.link_mut().with(move |l| {
            let sim = &l.host().game;
            let s = state::snapshot_world(&sim.game, &sim.events);
            (s.frame.rem_euclid(every as i32) == 0).then(|| s.to_json_line())
        })?;
        if let Some(line) = snap {
            writeln!(out, "{line}")?;
            snaps += 1;
        }
    }
    let notes = [format!(
        "{ran} server ticks, clock {STEP_MS} ms per step from {START_MS} ms, every {every}"
    )];
    writeln!(out, "{}", state::footer_line(snaps, &notes))?;
    out.flush()?;
    Ok(DumpReport { ticks: ran, snaps })
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
            units: single_player::client_unit_rows(archives)?,
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

/// The link the dump's bridge runs on.
pub type DumpLink = ThreadLink<Link<StepClock>>;

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
        ]))
        .unwrap();
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
            }
        );
        assert!(parse_args(&args(&["--out", "o"])).is_err(), "no --ticks");
        assert!(parse_args(&args(&["--ticks", "1"])).is_err(), "no --out");
        assert!(parse_args(&args(&["--ticks", "1", "--out", "o", "--every", "0"])).is_err());
        assert!(parse_args(&args(&["--ticks", "1", "--out", "o", "--date", "9.10.26"])).is_err());
        assert!(parse_args(&args(&["--ticks", "1", "--out", "o", "--frames", "3"])).is_err());
    }

    #[test]
    fn civil_dates() {
        assert_eq!(civil_date(0), "1970-01-01");
        assert_eq!(civil_date(11_016), "2000-02-29");
        assert_eq!(civil_date(20_735), "2026-10-09");
        assert!(is_date(&today()));
    }
}
