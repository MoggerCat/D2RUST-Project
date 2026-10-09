// Spec: specs/tools/poke.md
//! Pokes: directives that set game state directly at a chosen tick, for
//! checks that should not walk through the game to get there
//! (`tools/poke.md`). This module is a debug and test entry point
//! (§5 rule 1): nothing in normal play calls it; it has no I/O and draws
//! only what the creation path it calls draws (§1 rule 4).
//!
//! | Item | Does |
//! |---|---|
//! | [`Directive`], [`parse_directive`], `Display` | one directive (§1): strict parser and canonical writer |
//! | [`Spawn`] | the `spawn` line of a poke file (§1 rule 3, `scenario.md` §3.1) |
//! | [`PokeFile`] | a `poke 1` file (§2) |
//! | [`apply`], [`apply_op`], [`apply_line`] | resolve the references on the current state and run the directive on a [`WorldSim`] game (§5) |
//! | [`spawn_monster`] | the call sequences of a `spawn` step (`scenario.md` §3.1 rule 2) |
//! | [`GotoTarget`], [`GotoWalk`], [`goto_step`] | the `goto` walk, one step per tick (§6) |
//!
//! References (`scenario.md` §3 rule 3, §1 rule 1 here) are resolved by
//! [`apply`] on the state it is called on: the callers call it between
//! ticks t − 1 and t (§5 rule 2), so that is the state after tick t − 1.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt;

use crate::drlg::DrlgRoomId;
use crate::game::Game;
use crate::items::{ItemGame, ItemRequest, ItemTables};
use crate::missiles::param_flags as pf;
use crate::missiles::{create_missile, MissileParams};
use crate::monsters::init::{self, InitHost as _};
use crate::monsters::population::{placement, preset, spawn as pop_spawn};
use crate::units::{RoomId, UnitId, UnitType};
use crate::wiring::economy::{Economy, GameFields, ItemSpawn};
use crate::wiring::path::act_change;
use crate::wiring::path::place::{level_warp, place_unit};
use crate::wiring::path::PathCtx;
use crate::wiring::worldgen::dispatch::WorldSim;
use crate::wiring::worldgen::WorldPending;

/// Poke file format version (§2 rule 2).
pub const POKE_FORMAT_VERSION: u32 = 1;

/// Largest tick of a poke file line (as `scenario.md`'s `end`).
pub const MAX_TICK: u32 = 1_000_000;

/// Item qualities by name (`scenario.md` §2 item table; value = quality
/// 1 low … 8 crafted).
pub const QUALITIES: [&str; 8] = [
    "low", "normal", "superior", "magic", "set", "rare", "unique", "crafted",
];

/// A coordinate argument: absolute sub-tiles, or the player's position
/// ± N (`@x±N` / `@y±N`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Coord {
    Num(i32),
    X(i32),
    Y(i32),
}

/// A unit argument (`scenario.md` §3 rule 3, and `<type>/<guid>`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnitArg {
    /// `@player`.
    Player,
    /// `@<type>[:<class>][#n]`: the n-th unit (ascending GUID) of the
    /// type, of that class when given.
    Nth { ty: u8, class: Option<u32>, n: u32 },
    /// `@wp[#n]`: the n-th waypoint object.
    Waypoint(u32),
    /// `<type>/<guid>`.
    Guid { ty: u8, guid: u32 },
}

/// One directive (§1 rule 2 table; optional arguments `None` when not
/// written).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Directive {
    Object {
        class: u32,
        x: Coord,
        y: Coord,
        mode: Option<u32>,
    },
    Superunique {
        row: u32,
        x: Coord,
        y: Coord,
    },
    Missile {
        class: u32,
        x: Coord,
        y: Coord,
        tx: Coord,
        ty: Coord,
        /// (skill id, level).
        skill: Option<(u32, u32)>,
        owner: Option<UnitArg>,
    },
    SeedGame {
        lo: u32,
        hi: u32,
    },
    SeedUnit {
        unit: UnitArg,
        lo: u32,
        hi: u32,
    },
    Time {
        period: u32,
        ticks: u32,
    },
    Pos {
        unit: UnitArg,
        x: Coord,
        y: Coord,
    },
    Warp {
        level: u32,
        tile: Option<u32>,
    },
    Item {
        /// 3–4 characters `[a-z0-9]`.
        code: String,
        x: Coord,
        y: Coord,
        /// 1 low … 8 crafted ([`QUALITIES`]).
        quality: Option<u8>,
        ilvl: Option<u8>,
    },
    Stat {
        unit: UnitArg,
        stat: u16,
        layer: u16,
        value: i32,
    },
    State {
        unit: UnitArg,
        state: u16,
        on: bool,
    },
    Freeze {
        seconds: u32,
    },
    Goto(GotoTarget),
}

/// The target of a `goto` (§6 rule 1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GotoTarget {
    /// `preset <level>`: the goal level; `None` for `unit` (the player's
    /// level at the first step).
    pub preset: Option<u32>,
    /// Unit type: 1 monster or 2 object.
    pub ty: u8,
    /// Class (`monstats` / `objects` row).
    pub class: u32,
}

/// Steps after which a `goto` walk ends `failed` (§6 rule 3.4).
pub const GOTO_MAX_STEPS: u32 = 400;

/// The state a runner keeps between the steps of one `goto` (§6 rule 2).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GotoWalk {
    /// The goal level, fixed at the first step.
    pub goal: Option<u32>,
    /// DRLG rooms seen: (level id, tile x, tile y).
    pub seen: BTreeSet<(u32, i32, i32)>,
    /// DRLG rooms the player cannot stand in (no free cell, or the
    /// placement refused or landed elsewhere): the search avoids them.
    pub blocked: BTreeSet<(u32, i32, i32)>,
    /// Steps run so far.
    pub steps: u32,
}

/// The directive keywords, in the §1 table order.
pub const KEYWORDS: [&str; 13] = [
    "object",
    "superunique",
    "missile",
    "seed-game",
    "seed-unit",
    "time",
    "pos",
    "warp",
    "item",
    "stat",
    "state",
    "freeze",
    "goto",
];

impl Directive {
    /// The keyword (the trace record's `d`).
    pub fn keyword(&self) -> &'static str {
        match self {
            Self::Object { .. } => "object",
            Self::Superunique { .. } => "superunique",
            Self::Missile { .. } => "missile",
            Self::SeedGame { .. } => "seed-game",
            Self::SeedUnit { .. } => "seed-unit",
            Self::Time { .. } => "time",
            Self::Pos { .. } => "pos",
            Self::Warp { .. } => "warp",
            Self::Item { .. } => "item",
            Self::Stat { .. } => "stat",
            Self::State { .. } => "state",
            Self::Freeze { .. } => "freeze",
            Self::Goto(_) => "goto",
        }
    }
}

/// The monster kinds of a `spawn` line (`scenario.md` §3.1 rule 2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpawnKind {
    Normal,
    RandomBoss,
    Champion,
    Unique,
}

impl SpawnKind {
    pub const ALL: [SpawnKind; 4] = [Self::Normal, Self::RandomBoss, Self::Champion, Self::Unique];
    pub fn name(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::RandomBoss => "random-boss",
            Self::Champion => "champion",
            Self::Unique => "unique",
        }
    }
}

/// `spawn <class> <x> <y> <kind> [umod <id>...]` (§1 rule 3).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Spawn {
    /// monstats row.
    pub class: u32,
    pub x: Coord,
    pub y: Coord,
    pub kind: SpawnKind,
    pub umods: Vec<u8>,
}

/// What one poke file line runs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PokeOp {
    Directive(Directive),
    Spawn(Spawn),
}

/// One `at <tick> ...` line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PokeLine {
    /// Relative tick (§2 rule 4).
    pub tick: u32,
    pub op: PokeOp,
}

/// A parsed poke file (§2).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PokeFile {
    /// Non-decreasing ticks, file order within a tick.
    pub lines: Vec<PokeLine>,
}

/// A poke file line that does not parse (§2 rule 5).
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("line {line}: {message}")]
pub struct PokeError {
    /// 1-based line number (0: the file as a whole).
    pub line: usize,
    pub message: String,
}

// ---- numbers and references ----------------------------------------------

fn num(t: &str) -> Result<u32, String> {
    let r = match t.strip_prefix("0x") {
        Some(h) if !h.is_empty() && h.bytes().all(|b| b.is_ascii_hexdigit()) => {
            u32::from_str_radix(h, 16)
        }
        Some(_) => return Err(format!("bad number {t:?}")),
        None if !t.is_empty() && t.bytes().all(|b| b.is_ascii_digit()) => t.parse(),
        None => return Err(format!("bad number {t:?}")),
    };
    r.map_err(|_| format!("number {t:?} does not fit 32 bits"))
}

fn ranged(t: &str, lo: u32, hi: u32, what: &str) -> Result<u32, String> {
    let v = num(t)?;
    if v < lo || v > hi {
        return Err(format!("{what} {v} outside {lo}..={hi}"));
    }
    Ok(v)
}

fn signed(t: &str) -> Result<i32, String> {
    match t.strip_prefix('-') {
        Some(rest) => {
            let v = i64::from(num(rest)?);
            i32::try_from(-v).map_err(|_| format!("number {t:?} does not fit i32"))
        }
        None => i32::try_from(num(t)?).map_err(|_| format!("number {t:?} does not fit i32")),
    }
}

fn coord(t: &str) -> Result<Coord, String> {
    let Some(body) = t.strip_prefix('@') else {
        return signed(t).map(Coord::Num);
    };
    for (axis, mk) in [("x", Coord::X as fn(i32) -> Coord), ("y", Coord::Y)] {
        if let Some(rest) = body.strip_prefix(axis) {
            let d = match rest.as_bytes().first() {
                None => 0,
                Some(b'+') if !rest[1..].starts_with('-') => signed(&rest[1..])?,
                Some(b'-') if rest.len() > 1 => signed(rest)?,
                _ => return Err(format!("bad position {t:?}")),
            };
            if d == 0 && !rest.is_empty() {
                return Err(format!("{t:?}: write a zero offset as @{axis}"));
            }
            return Ok(mk(d));
        }
    }
    Err(format!("{t:?}: a position is a number, @x±N or @y±N"))
}

fn unit_arg(t: &str) -> Result<UnitArg, String> {
    if let Some((ty, guid)) = t.split_once('/') {
        let ty = ranged(ty, 0, 5, "unit type")? as u8;
        return Ok(UnitArg::Guid {
            ty,
            guid: num(guid)?,
        });
    }
    let Some(body) = t.strip_prefix('@') else {
        return Err(format!(
            "{t:?}: a unit is @player, @<type>[:<class>][#n], @wp[#n] or <type>/<guid>"
        ));
    };
    if body == "player" {
        return Ok(UnitArg::Player);
    }
    let (head, n) = match body.split_once('#') {
        Some((h, n)) => {
            let n = num(n)?;
            if n == 0 {
                return Err(format!("{t:?}: write #0 as nothing"));
            }
            (h, n)
        }
        None => (body, 0),
    };
    if head == "wp" {
        return Ok(UnitArg::Waypoint(n));
    }
    let (ty, class) = match head.split_once(':') {
        Some((ty, c)) => (ty, Some(num(c)?)),
        None => (head, None),
    };
    let ty = ranged(ty, 0, 5, "unit type").map_err(|e| {
        format!("{t:?}: {e}; a unit is @player, @<type>[:<class>][#n], @wp[#n] or <type>/<guid>")
    })? as u8;
    Ok(UnitArg::Nth { ty, class, n })
}

impl fmt::Display for Coord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let off = |f: &mut fmt::Formatter<'_>, axis: &str, d: i32| match d {
            0 => write!(f, "@{axis}"),
            d if d > 0 => write!(f, "@{axis}+{d}"),
            d => write!(f, "@{axis}-{}", d.unsigned_abs()),
        };
        match *self {
            Self::Num(v) => write!(f, "{v}"),
            Self::X(d) => off(f, "x", d),
            Self::Y(d) => off(f, "y", d),
        }
    }
}

impl fmt::Display for UnitArg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let nth = |f: &mut fmt::Formatter<'_>, n: u32| {
            if n == 0 {
                Ok(())
            } else {
                write!(f, "#{n}")
            }
        };
        match *self {
            Self::Player => write!(f, "@player"),
            Self::Nth { ty, class, n } => {
                write!(f, "@{ty}")?;
                if let Some(c) = class {
                    write!(f, ":{c}")?;
                }
                nth(f, n)
            }
            Self::Waypoint(n) => {
                write!(f, "@wp")?;
                nth(f, n)
            }
            Self::Guid { ty, guid } => write!(f, "{ty}/{guid}"),
        }
    }
}

// ---- directives -----------------------------------------------------------

/// Parses one directive from its tokens (keyword first; §1). Errors name
/// the problem; the caller adds the line.
pub fn parse_directive(toks: &[&str]) -> Result<Directive, String> {
    let Some((&kw, args)) = toks.split_first() else {
        return Err("empty directive".into());
    };
    // Fixed arguments, then the optional `key value...` groups.
    let fixed = |n: usize, usage: &str| -> Result<(&[&str], &[&str]), String> {
        if args.len() < n {
            return Err(format!("`{kw}` needs {n} argument(s): `{kw} {usage}`"));
        }
        Ok(args.split_at(n))
    };
    let exact = |n: usize, usage: &str| -> Result<&[&str], String> {
        if args.len() != n {
            return Err(format!(
                "`{kw}` takes {n} argument(s), got {}: `{kw} {usage}`",
                args.len()
            ));
        }
        Ok(args)
    };
    fn once<T>(slot: &mut Option<T>, v: T, what: &str) -> Result<(), String> {
        if slot.is_some() {
            return Err(format!("`{what}` given twice"));
        }
        *slot = Some(v);
        Ok(())
    }
    let d = match kw {
        "object" => {
            let usage = "<class> <x> <y> [mode <m>]";
            let (a, mut rest) = fixed(3, usage)?;
            let mut mode = None;
            while let [key, r @ ..] = rest {
                match (*key, r) {
                    ("mode", [m, r @ ..]) => {
                        once(&mut mode, ranged(m, 0, 7, "object mode")?, "mode")?;
                        rest = r;
                    }
                    _ => return Err(format!("`object {usage}`: unexpected {key:?}")),
                }
            }
            Directive::Object {
                class: ranged(a[0], 0, 0xFFFF, "objects row")?,
                x: coord(a[1])?,
                y: coord(a[2])?,
                mode,
            }
        }
        "superunique" => {
            let a = exact(3, "<row> <x> <y>")?;
            Directive::Superunique {
                row: ranged(a[0], 0, 0xFFFF, "superuniques row")?,
                x: coord(a[1])?,
                y: coord(a[2])?,
            }
        }
        "missile" => {
            let usage = "<class> <x> <y> <tx> <ty> [skill <id> <level>] [owner <ref>]";
            let (a, mut rest) = fixed(5, usage)?;
            let (mut skill, mut owner) = (None, None);
            while let [key, r @ ..] = rest {
                match (*key, r) {
                    ("skill", [id, lv, r @ ..]) => {
                        let s = (
                            ranged(id, 0, 0xFFFF, "skill id")?,
                            ranged(lv, 0, 255, "skill level")?,
                        );
                        once(&mut skill, s, "skill")?;
                        rest = r;
                    }
                    ("owner", [u, r @ ..]) => {
                        once(&mut owner, unit_arg(u)?, "owner")?;
                        rest = r;
                    }
                    _ => return Err(format!("`missile {usage}`: unexpected {key:?}")),
                }
            }
            Directive::Missile {
                class: ranged(a[0], 0, 0xFFFF, "missiles row")?,
                x: coord(a[1])?,
                y: coord(a[2])?,
                tx: coord(a[3])?,
                ty: coord(a[4])?,
                skill,
                owner,
            }
        }
        "seed-game" => {
            let a = exact(2, "<lo> <hi>")?;
            Directive::SeedGame {
                lo: num(a[0])?,
                hi: num(a[1])?,
            }
        }
        "seed-unit" => {
            let a = exact(3, "<ref> <lo> <hi>")?;
            Directive::SeedUnit {
                unit: unit_arg(a[0])?,
                lo: num(a[1])?,
                hi: num(a[2])?,
            }
        }
        "time" => {
            let a = exact(2, "<period 0..5> <ticks>")?;
            Directive::Time {
                period: ranged(a[0], 0, 5, "period")?,
                ticks: num(a[1])?,
            }
        }
        "pos" => {
            let a = exact(3, "<ref> <x> <y>")?;
            Directive::Pos {
                unit: unit_arg(a[0])?,
                x: coord(a[1])?,
                y: coord(a[2])?,
            }
        }
        "warp" => {
            let usage = "<level> [tile <n>]";
            let (a, rest) = fixed(1, usage)?;
            let tile = match rest {
                [] => None,
                ["tile", n] => Some(ranged(n, 0, 0xFF, "tile index")?),
                _ => return Err(format!("`warp {usage}`")),
            };
            Directive::Warp {
                level: ranged(a[0], 0, 0xFFFF, "level id")?,
                tile,
            }
        }
        "item" => {
            let usage = "<code> <x> <y> [quality <q>] [ilvl <n>]";
            let (a, mut rest) = fixed(3, usage)?;
            let code = a[0];
            if !(3..=4).contains(&code.len())
                || !code
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
            {
                return Err(format!("item code {code:?}: 3-4 of [a-z0-9]"));
            }
            let (mut quality, mut ilvl) = (None, None);
            while let [key, r @ ..] = rest {
                match (*key, r) {
                    ("quality", [q, r @ ..]) => {
                        let i = QUALITIES
                            .iter()
                            .position(|n| n == q)
                            .ok_or_else(|| format!("unknown quality {q:?}"))?;
                        once(&mut quality, i as u8 + 1, "quality")?;
                        rest = r;
                    }
                    ("ilvl", [n, r @ ..]) => {
                        once(&mut ilvl, ranged(n, 1, 99, "ilvl")? as u8, "ilvl")?;
                        rest = r;
                    }
                    _ => return Err(format!("`item {usage}`: unexpected {key:?}")),
                }
            }
            Directive::Item {
                code: code.to_owned(),
                x: coord(a[1])?,
                y: coord(a[2])?,
                quality,
                ilvl,
            }
        }
        "stat" => {
            let a = exact(4, "<ref> <stat> <layer> <i32>")?;
            Directive::Stat {
                unit: unit_arg(a[0])?,
                stat: ranged(a[1], 0, 0xFFFF, "stat id")? as u16,
                layer: ranged(a[2], 0, 0xFFFF, "layer")? as u16,
                value: signed(a[3])?,
            }
        }
        "state" => {
            let a = exact(3, "<ref> <state> on|off")?;
            let on = match a[2] {
                "on" => true,
                "off" => false,
                v => return Err(format!("state {v:?}: on or off")),
            };
            Directive::State {
                unit: unit_arg(a[0])?,
                state: ranged(a[1], 0, 0xFFFF, "state id")? as u16,
                on,
            }
        }
        "freeze" => {
            let a = exact(1, "<seconds>")?;
            Directive::Freeze {
                seconds: ranged(a[0], 0, 3600, "seconds")?,
            }
        }
        "goto" => {
            let usage = "unit [<type>:]<class> | preset <level> [<type>:]<class>";
            let (preset, id) = match args {
                ["unit", id] => (None, *id),
                ["preset", lv, id] => (Some(ranged(lv, 0, 0xFFFF, "level id")?), *id),
                _ => return Err(format!("`goto {usage}`")),
            };
            let (ty, class) = match id.split_once(':') {
                Some((t, c)) => (ranged(t, 1, 2, "goto unit type (1 monster, 2 object)")?, c),
                None => (1, id),
            };
            Directive::Goto(GotoTarget {
                preset,
                ty: ty as u8,
                class: ranged(class, 0, 0xFFFF, "class")?,
            })
        }
        k => {
            return Err(format!(
                "unknown directive {k:?}: one of {}",
                KEYWORDS.join(", ")
            ))
        }
    };
    Ok(d)
}

/// Parses a directive line (`<keyword> <args>...`, tokens separated by
/// spaces and tabs).
pub fn parse_directive_text(line: &str) -> Result<Directive, String> {
    let toks: Vec<&str> = line.split_ascii_whitespace().collect();
    parse_directive(&toks)
}

impl fmt::Display for Directive {
    /// The canonical text (§3 rule 2): the directive and its arguments as
    /// parsed, numbers decimal, optional arguments in the table order.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.keyword())?;
        match self {
            Self::Object { class, x, y, mode } => {
                write!(f, " {class} {x} {y}")?;
                if let Some(m) = mode {
                    write!(f, " mode {m}")?;
                }
            }
            Self::Superunique { row, x, y } => write!(f, " {row} {x} {y}")?,
            Self::Missile {
                class,
                x,
                y,
                tx,
                ty,
                skill,
                owner,
            } => {
                write!(f, " {class} {x} {y} {tx} {ty}")?;
                if let Some((s, l)) = skill {
                    write!(f, " skill {s} {l}")?;
                }
                if let Some(o) = owner {
                    write!(f, " owner {o}")?;
                }
            }
            Self::SeedGame { lo, hi } => write!(f, " {lo} {hi}")?,
            Self::SeedUnit { unit, lo, hi } => write!(f, " {unit} {lo} {hi}")?,
            Self::Time { period, ticks } => write!(f, " {period} {ticks}")?,
            Self::Pos { unit, x, y } => write!(f, " {unit} {x} {y}")?,
            Self::Warp { level, tile } => {
                write!(f, " {level}")?;
                if let Some(t) = tile {
                    write!(f, " tile {t}")?;
                }
            }
            Self::Item {
                code,
                x,
                y,
                quality,
                ilvl,
            } => {
                write!(f, " {code} {x} {y}")?;
                if let Some(q) = quality {
                    write!(f, " quality {}", QUALITIES[usize::from(*q) - 1])?;
                }
                if let Some(l) = ilvl {
                    write!(f, " ilvl {l}")?;
                }
            }
            Self::Stat {
                unit,
                stat,
                layer,
                value,
            } => write!(f, " {unit} {stat} {layer} {value}")?,
            Self::State { unit, state, on } => {
                write!(f, " {unit} {state} {}", if *on { "on" } else { "off" })?
            }
            Self::Freeze { seconds } => write!(f, " {seconds}")?,
            Self::Goto(t) => match t.preset {
                Some(l) => write!(f, " preset {l} {}:{}", t.ty, t.class)?,
                None => write!(f, " unit {}:{}", t.ty, t.class)?,
            },
        }
        Ok(())
    }
}

/// Parses `spawn`'s arguments (after the keyword): `<class> <x> <y>
/// <kind> [umod <id>...]` (`scenario.md` §3.1).
pub fn parse_spawn(toks: &[&str]) -> Result<Spawn, String> {
    let [class, x, y, kind, rest @ ..] = toks else {
        return Err("`spawn <class> <x> <y> <kind> [umod <id>...]`".into());
    };
    let kind = SpawnKind::ALL
        .into_iter()
        .find(|k| k.name() == *kind)
        .ok_or_else(|| {
            format!("unknown spawn kind {kind:?}: normal, random-boss, champion or unique")
        })?;
    let umods = match rest {
        [] => Vec::new(),
        ["umod", ids @ ..] if !ids.is_empty() => ids
            .iter()
            .map(|t| ranged(t, 1, 255, "umod").map(|v| v as u8))
            .collect::<Result<Vec<u8>, _>>()?,
        _ => return Err("after the kind: `umod <id>...`".into()),
    };
    let ok = match kind {
        SpawnKind::Normal | SpawnKind::RandomBoss => umods.is_empty(),
        SpawnKind::Champion => umods.len() == 1,
        SpawnKind::Unique => (1..=9).contains(&umods.len()),
    };
    if !ok {
        return Err(format!(
            "spawn {}: umods: none for normal and random-boss, one for champion, 1-9 for unique",
            kind.name()
        ));
    }
    Ok(Spawn {
        class: num(class)?,
        x: coord(x)?,
        y: coord(y)?,
        kind,
        umods,
    })
}

impl fmt::Display for Spawn {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "spawn {} {} {} {}",
            self.class,
            self.x,
            self.y,
            self.kind.name()
        )?;
        if !self.umods.is_empty() {
            write!(f, " umod")?;
            for u in &self.umods {
                write!(f, " {u}")?;
            }
        }
        Ok(())
    }
}

impl fmt::Display for PokeOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Directive(d) => d.fmt(f),
            Self::Spawn(s) => s.fmt(f),
        }
    }
}

/// Parses `<directive> <args>...` or `spawn ...`.
pub fn parse_op(toks: &[&str]) -> Result<PokeOp, String> {
    match toks.split_first() {
        Some((&"spawn", rest)) => parse_spawn(rest).map(PokeOp::Spawn),
        _ => parse_directive(toks).map(PokeOp::Directive),
    }
}

impl PokeFile {
    /// Parses a `poke 1` file (§2).
    pub fn parse(text: &str) -> Result<Self, PokeError> {
        let mut version_seen = false;
        let mut lines: Vec<PokeLine> = Vec::new();
        for (k, raw) in text.lines().enumerate() {
            let line = k + 1;
            let toks: Vec<&str> = raw
                .split_ascii_whitespace()
                .take_while(|t| !t.starts_with('#'))
                .collect();
            if toks.is_empty() {
                continue;
            }
            let err = |message: String| PokeError { line, message };
            if !version_seen {
                if toks != ["poke", "1"] {
                    return Err(err(match toks.as_slice() {
                        ["poke", v] => format!("unknown poke format version {v}"),
                        _ => "the first line must be `poke 1`".into(),
                    }));
                }
                version_seen = true;
                continue;
            }
            let ["at", tick, rest @ ..] = toks.as_slice() else {
                return Err(err("`at <tick> <directive> <args>...`".into()));
            };
            let tick = ranged(tick, 0, MAX_TICK, "tick").map_err(err)?;
            if let Some(prev) = lines.last() {
                if tick < prev.tick {
                    return Err(err(format!(
                        "line at tick {tick} after a line at tick {}",
                        prev.tick
                    )));
                }
            }
            let op = parse_op(rest).map_err(err)?;
            lines.push(PokeLine { tick, op });
        }
        if !version_seen {
            return Err(PokeError {
                line: 0,
                message: "empty poke file: no `poke 1` line".into(),
            });
        }
        Ok(Self { lines })
    }

    /// The canonical text.
    pub fn to_text(&self) -> String {
        let mut s = format!("poke {POKE_FORMAT_VERSION}\n");
        for l in &self.lines {
            s.push_str(&format!("at {} {}\n", l.tick, l.op));
        }
        s
    }
}

// ---- apply ----------------------------------------------------------------

/// A directive's result (§3 rule 3: the trace record's `r`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PokeResult {
    /// Done; the GUID of the unit it created, if any.
    Ok(Option<u32>),
    /// The game's own function refused (placement, class check, no room).
    Failed,
    /// A reference matched no unit: the reference as written.
    Unresolved(String),
    /// This side cannot run the directive: why.
    Gap(String),
    /// A `goto` step that did not land yet (§6 rule 3.3): run the next
    /// step after the next tick.
    Pending,
}

impl PokeResult {
    /// `ok`, `failed`, `unresolved` or `gap`.
    pub fn code(&self) -> &'static str {
        match self {
            Self::Ok(_) => "ok",
            Self::Failed => "failed",
            Self::Unresolved(_) => "unresolved",
            Self::Gap(_) => "gap",
            Self::Pending => "pending",
        }
    }

    /// The created unit's GUID (only with `ok`).
    pub fn guid(&self) -> Option<u32> {
        match self {
            Self::Ok(g) => *g,
            _ => None,
        }
    }
}

static NO_WAYPOINTS: BTreeSet<u32> = BTreeSet::new();

/// What [`apply`] needs besides the game.
#[derive(Clone, Copy)]
pub struct Env<'a> {
    /// The character (`@player`, `@x`, `@y`, the default missile owner,
    /// the act of `time`, the unit `warp` moves).
    pub player: UnitId,
    /// `objects` rows with operate function 23 (`@wp`, `world/waypoints.md`
    /// §5); empty: `@wp` never resolves.
    pub waypoint_classes: &'a BTreeSet<u32>,
    /// The item tables `item` creates from; `None`: `item` is a gap.
    pub items: Option<&'a ItemTables>,
}

impl<'a> Env<'a> {
    /// No waypoint classes, no item tables.
    pub fn new(player: UnitId) -> Self {
        Self {
            player,
            waypoint_classes: &NO_WAYPOINTS,
            items: None,
        }
    }
}

/// The player's path position.
fn player_pos<X: WorldPending>(sim: &WorldSim<X>, env: &Env<'_>) -> (i32, i32) {
    sim.action.sys.hooks.path_position(env.player)
}

fn resolve_coord<X: WorldPending>(
    c: Coord,
    sim: &WorldSim<X>,
    env: &Env<'_>,
) -> Result<i32, String> {
    match c {
        Coord::Num(v) => Ok(v),
        Coord::X(d) => player_pos(sim, env)
            .0
            .checked_add(d)
            .ok_or_else(|| c.to_string()),
        Coord::Y(d) => player_pos(sim, env)
            .1
            .checked_add(d)
            .ok_or_else(|| c.to_string()),
    }
}

/// Resolves a unit argument on the current state (`scenario.md` §3 rules
/// 3–4: every unit of the unit lists, ascending GUID order); `Err` holds
/// the reference as written.
pub fn resolve_unit<X: WorldPending>(
    u: UnitArg,
    game: &Game,
    sim: &WorldSim<X>,
    env: &Env<'_>,
) -> Result<UnitId, String> {
    let lists = &game.lists;
    let nth = |ty: UnitType, keep: &dyn Fn(UnitId) -> bool, n: u32| {
        let mut v: Vec<(u32, UnitId)> = lists
            .units_of_type(ty)
            .into_iter()
            .filter(|&id| keep(id))
            .filter_map(|id| lists.unit(id).map(|e| (e.guid, id)))
            .collect();
        v.sort_unstable();
        v.get(n as usize).map(|&(_, id)| id)
    };
    let class = |id: UnitId| sim.action.sys.units.get(id).map(|r| r.class);
    let found = match u {
        UnitArg::Player => lists.unit(env.player).map(|_| env.player),
        UnitArg::Nth { ty, class: c, n } => nth(
            UnitType::ALL[usize::from(ty)],
            &|id| c.is_none_or(|c| class(id) == Some(c)),
            n,
        ),
        UnitArg::Waypoint(n) => nth(
            UnitType::Object,
            &|id| class(id).is_some_and(|c| env.waypoint_classes.contains(&c)),
            n,
        ),
        UnitArg::Guid { ty, guid } => lists.find_unit(UnitType::ALL[usize::from(ty)], guid),
    };
    found.ok_or_else(|| u.to_string())
}

/// The room holding (x, y): the room of a point from `near` (the
/// unit's room and its neighbours, `0x00463740`).
fn room_near<X: WorldPending>(
    game: &Game,
    sim: &WorldSim<X>,
    near: UnitId,
    x: i32,
    y: i32,
) -> Option<RoomId> {
    let r = game.lists.unit(near)?.room()?;
    sim.action.sys.hooks.drlg.find_room(game, r, x, y)
}

fn guid_of(game: &Game, u: UnitId) -> Option<u32> {
    game.lists.unit(u).map(|e| e.guid)
}

fn created(game: &Game, u: Option<UnitId>) -> PokeResult {
    match u.and_then(|u| guid_of(game, u)) {
        Some(g) => PokeResult::Ok(Some(g)),
        None => PokeResult::Failed,
    }
}

/// Runs `d` on the game (§1, §5): resolves its references on the current
/// state, then calls the d2rs path of its row. No I/O; it draws only what
/// that path draws.
pub fn apply<X: WorldPending>(
    game: &mut Game,
    sim: &mut WorldSim<X>,
    env: &Env<'_>,
    d: &Directive,
) -> PokeResult {
    match run(game, sim, env, d) {
        Ok(r) => r,
        Err(reference) => PokeResult::Unresolved(reference),
    }
}

/// [`apply`] for a poke file line (a directive or a `spawn`).
pub fn apply_op<X: WorldPending>(
    game: &mut Game,
    sim: &mut WorldSim<X>,
    env: &Env<'_>,
    op: &PokeOp,
) -> PokeResult {
    match op {
        PokeOp::Directive(d) => apply(game, sim, env, d),
        PokeOp::Spawn(sp) => {
            let pos = resolve_coord(sp.x, sim, env)
                .and_then(|x| resolve_coord(sp.y, sim, env).map(|y| (x, y)));
            let (x, y) = match pos {
                Ok(p) => p,
                Err(r) => return PokeResult::Unresolved(r),
            };
            let act = game
                .lists
                .unit(env.player)
                .and_then(|e| e.room())
                .and_then(|r| game.lists.room(r))
                .map_or(0, |r| r.act);
            let u = spawn_monster(game, sim, act, sp.class, x, y, sp.kind, &sp.umods);
            created(game, u)
        }
    }
}

/// Parses `line` (`<directive> <args>...` or `spawn ...`) and applies
/// it: the one-call entry point of the play and dump tools.
pub fn apply_line<X: WorldPending>(
    game: &mut Game,
    sim: &mut WorldSim<X>,
    env: &Env<'_>,
    line: &str,
) -> Result<PokeResult, String> {
    let toks: Vec<&str> = line.split_ascii_whitespace().collect();
    let op = parse_op(&toks)?;
    Ok(apply_op(game, sim, env, &op))
}

fn run<X: WorldPending>(
    game: &mut Game,
    sim: &mut WorldSim<X>,
    env: &Env<'_>,
    d: &Directive,
) -> Result<PokeResult, String> {
    let c = |v: Coord, sim: &WorldSim<X>| resolve_coord(v, sim, env);
    Ok(match d {
        Directive::Object { class, x, y, mode } => {
            let (x, y) = (c(*x, sim)?, c(*y, sim)?);
            let Some(room) = room_near(game, sim, env.player, x, y) else {
                return Ok(PokeResult::Failed);
            };
            // Allocator 0x00555230 with type 2, flags 1 (add), mode as
            // given (default 0), with the monster state lent (an InitFn
            // may spawn monsters, edge case 2): the creation the game's
            // own objects go through (`View::create_object`, as the
            // population and the quests), so the per-kind init
            // (`objects.md` §3: control record, InitFn) runs on the
            // allocation's room and (x, y) before `SUNIT_Add`
            // (`units.md` §3.1 steps 7–8). A game without object state,
            // a mode beyond a byte or a class past the objects rows: the
            // bare allocation (the init dispatch's own checks decide).
            let class = *class;
            let mode = mode.unwrap_or(0);
            let u = sim.lend(|a| {
                a.with(game, |g, v| match u8::try_from(mode) {
                    Ok(m)
                        if v.h.objects.is_some()
                            && class <= u32::from(crate::world::objects::CLASS_BOUND) =>
                    {
                        v.create_object(g, room, class, x, y, m)
                    }
                    _ => {
                        let req = crate::units::lifecycle::AllocRequest {
                            ty: UnitType::Object,
                            class,
                            room: Some(room),
                            add: true,
                            fixed_guid: None,
                            mode,
                            allied: false,
                        };
                        v.allocate(g, &req, x, y)
                    }
                })
            });
            created(game, u)
        }
        Directive::Superunique { row, x, y } => {
            let (x, y) = (c(*x, sim)?, c(*y, sim)?);
            let Some(room) = room_near(game, sim, env.player, x, y) else {
                return Ok(PokeResult::Failed);
            };
            let su = *row as i32;
            let u = sim.population(game, |cx| preset::superunique(cx, room, x, y, su));
            created(game, u)
        }
        Directive::Missile {
            class,
            x,
            y,
            tx,
            ty,
            skill,
            owner,
        } => {
            let (x, y, tx, ty) = (c(*x, sim)?, c(*y, sim)?, c(*tx, sim)?, c(*ty, sim)?);
            let owner = match owner {
                Some(o) => resolve_unit(*o, game, sim, env)?,
                None => env.player,
            };
            let (skill, level) = skill.unwrap_or((0, 0));
            let p = MissileParams {
                flags: pf::POSITION | pf::TARGET_ABSOLUTE,
                owner: Some(owner),
                origin: Some(owner),
                class: *class as i32,
                x,
                y,
                target_x: tx,
                target_y: ty,
                skill: skill as i32,
                level: level as i32,
                ..MissileParams::default()
            };
            let u = sim
                .lend(|a| a.missiles(game, |g, cx| create_missile(g, cx, &p)))
                .flatten();
            created(game, u)
        }
        Directive::SeedGame { lo, hi } => {
            sim.action.sys.hooks.game_seed.set(*lo, *hi);
            PokeResult::Ok(None)
        }
        Directive::SeedUnit { unit, lo, hi } => {
            let u = resolve_unit(*unit, game, sim, env)?;
            match sim.action.sys.units.get_mut(u) {
                Some(r) => {
                    r.seed.set(*lo, *hi);
                    PokeResult::Ok(None)
                }
                None => PokeResult::Failed,
            }
        }
        Directive::Time { period, ticks } => {
            let act = game
                .lists
                .unit(env.player)
                .and_then(|e| e.room())
                .and_then(|r| game.lists.room(r))
                .map(|r| r.act);
            match act.and_then(|a| game.lists.act_mut(a)) {
                Some(a) => {
                    a.environment.period = *period;
                    a.environment.ticks = *ticks;
                    PokeResult::Ok(None)
                }
                None => PokeResult::Failed,
            }
        }
        Directive::Pos { unit, x, y } => {
            let u = resolve_unit(*unit, game, sim, env)?;
            let (x, y) = (c(*x, sim)?, c(*y, sim)?);
            if !sim.action.sys.hooks.path_has(u) {
                return Ok(PokeResult::Failed);
            }
            let Some(room) = room_near(game, sim, u, x, y) else {
                return Ok(PokeResult::Failed);
            };
            sim.lend(|a| a.with(game, |g, v| PathCtx::of(v, g).teleport(u, Some(room), x, y)));
            PokeResult::Ok(None)
        }
        Directive::Warp { level, tile } => {
            let player = env.player;
            let tile = tile.unwrap_or(0);
            // `0x0053AEC0` (`waypoints.md` §7 rule 5): the same-act warp,
            // else the act change `0x0053ACC0` (§11).
            let r = sim.lend(|a| {
                a.with(game, |g, v| {
                    level_warp(PathCtx::of(v, g), player, *level, tile)
                        .unwrap_or_else(|| act_change::run(PathCtx::of(v, g), player, *level, tile))
                })
            });
            if r {
                PokeResult::Ok(None)
            } else {
                PokeResult::Failed
            }
        }
        Directive::Item {
            code,
            x,
            y,
            quality,
            ilvl,
        } => {
            let Some(tables) = env.items else {
                return Ok(PokeResult::Gap(
                    "item: no item tables in this runner".into(),
                ));
            };
            let (x, y) = (c(*x, sim)?, c(*y, sim)?);
            let mut key = [b' '; 4];
            key[..code.len()].copy_from_slice(code.as_bytes());
            let Some(index) = tables.find_code(key) else {
                return Ok(PokeResult::Failed);
            };
            let Some(room) = room_near(game, sim, env.player, x, y) else {
                return Ok(PokeResult::Failed);
            };
            let u = sim.lend(|a| {
                a.with(game, |g, v| {
                    create_ground_item(g, v, tables, room, index, *quality, *ilvl, x, y)
                })
            });
            created(game, u)
        }
        Directive::Stat {
            unit,
            stat,
            layer,
            value,
        } => {
            let u = resolve_unit(*unit, game, sim, env)?;
            let sys = &mut sim.action.sys;
            if sys.stats.unit_list(u).is_none() {
                return Ok(PokeResult::Failed);
            }
            sys.stats.unit_set(&mut sys.hooks, u, *stat, *value, *layer);
            PokeResult::Ok(None)
        }
        Directive::State { unit, state, on } => {
            let u = resolve_unit(*unit, game, sim, env)?;
            let sys = &sim.action.sys;
            if usize::from(*state) >= sys.stats.data().states.count()
                || sys.stats.unit_list(u).is_none()
            {
                return Ok(PokeResult::Failed);
            }
            // `0x00639DB0`: the toggle, then the update-queue insert.
            let queued = sim.action.with(game, |g, v| {
                v.set_state(u, *state, *on);
                g.lists.queue_update(u).is_ok()
            });
            if queued {
                PokeResult::Ok(None)
            } else {
                PokeResult::Failed
            }
        }
        Directive::Freeze { .. } => PokeResult::Ok(None),
        Directive::Goto(t) => goto_step(game, sim, env, t, &mut GotoWalk::default()),
    })
}

/// The level id of the room holding unit `u`.
fn unit_level<X: WorldPending>(game: &Game, sim: &WorldSim<X>, u: UnitId) -> Option<u32> {
    let room = game.lists.unit(u)?.room()?;
    sim.action.sys.hooks.drlg.level_id(game, room)
}

/// One step of a `goto` walk (§6 rule 3): `Pending` until the target is
/// found and the player placed next to it (`Ok` with the target's GUID)
/// or the walk ends `Failed`. `walk` is the state the runner keeps
/// between steps (a fresh one for a new `goto`).
pub fn goto_step<X: WorldPending>(
    game: &mut Game,
    sim: &mut WorldSim<X>,
    env: &Env<'_>,
    t: &GotoTarget,
    walk: &mut GotoWalk,
) -> PokeResult {
    let player = env.player;
    walk.steps += 1;
    if walk.steps > GOTO_MAX_STEPS {
        return PokeResult::Failed;
    }
    let Some(here) = unit_level(game, sim, player) else {
        return PokeResult::Failed;
    };
    let goal = *walk.goal.get_or_insert(t.preset.unwrap_or(here));
    // Rule 3.1: the warp to a preset's level, at the first step only.
    if walk.steps == 1 && here != goal {
        let warp = Directive::Warp {
            level: goal,
            tile: None,
        };
        return match apply(game, sim, env, &warp) {
            PokeResult::Ok(_) => PokeResult::Pending,
            _ => PokeResult::Failed,
        };
    }
    // Rule 3.2: the first unit of the target's type and class in the goal
    // level, ascending GUID.
    let ty = UnitType::ALL[usize::from(t.ty)];
    let mut found: Vec<(u32, UnitId)> = game
        .lists
        .units_of_type(ty)
        .into_iter()
        .filter(|&u| {
            sim.action
                .sys
                .units
                .get(u)
                .is_some_and(|r| r.class == t.class)
        })
        .filter(|&u| unit_level(game, sim, u) == Some(goal))
        .filter_map(|u| guid_of(game, u).map(|g| (g, u)))
        .collect();
    found.sort_unstable();
    if let Some(&(guid, target)) = found.first() {
        let Some(room) = game.lists.unit(target).and_then(|e| e.room()) else {
            return PokeResult::Failed;
        };
        let (x, y) = sim.action.sys.hooks.path_position(target);
        let placed = sim.lend(|a| {
            a.with(game, |g, v| {
                place_unit(PathCtx::of(v, g), player, Some(room), x, y, false, false)
            })
        });
        return if placed {
            PokeResult::Ok(Some(guid))
        } else {
            PokeResult::Failed
        };
    }
    // Rule 3.3: mark what is seen, then one hop towards the nearest room
    // of the goal level not seen.
    let Some(room) = game.lists.unit(player).and_then(|e| e.room()) else {
        return PokeResult::Failed;
    };
    let hop = {
        let Some((d, cur)) = sim.action.sys.hooks.drlg.drlg_room(game, room) else {
            return PokeResult::Failed;
        };
        let key = |r: DrlgRoomId| {
            let dr = d.room(r);
            (d.level(dr.level).id, dr.rect.x, dr.rect.y)
        };
        let near = |r: DrlgRoomId| d.room(r).near().unwrap_or(&[]).to_vec();
        walk.seen.insert(key(cur));
        for n in near(cur) {
            if d.room(n).active().is_some() {
                walk.seen.insert(key(n));
            }
        }
        // Breadth first over the near arrays, in their stored order.
        let mut parent: BTreeMap<DrlgRoomId, DrlgRoomId> = BTreeMap::new();
        let mut queue = VecDeque::from([cur]);
        let mut done = BTreeSet::from([cur]);
        let mut goal_room = None;
        while let Some(r) = queue.pop_front() {
            let (lv, _, _) = key(r);
            if lv == goal && !walk.seen.contains(&key(r)) {
                goal_room = Some(r);
                break;
            }
            for n in near(r) {
                if !walk.blocked.contains(&key(n)) && done.insert(n) {
                    parent.insert(n, r);
                    queue.push_back(n);
                }
            }
        }
        let Some(mut h) = goal_room else {
            return PokeResult::Failed;
        };
        while let Some(&p) = parent.get(&h) {
            if p == cur {
                break;
            }
            h = p;
        }
        let Some(a) = d.room(h).active() else {
            return PokeResult::Failed;
        };
        // The free cell of H nearest its centre (first found on a tie).
        let r = a.subtiles;
        let (cx, cy) = (r.x + r.w / 2, r.y + r.h / 2);
        let mut best: Option<(i64, i32, i32)> = None;
        for y in r.y..r.y + r.h {
            for x in r.x..r.x + r.w {
                let free = a
                    .collision
                    .get(x, y)
                    .is_some_and(|m| m & crate::path::collision::masks::PLAYER_MOVE == 0);
                let d2 = i64::from(x - cx).pow(2) + i64::from(y - cy).pow(2);
                if free && best.is_none_or(|(b, _, _)| d2 < b) {
                    best = Some((d2, x, y));
                }
            }
        }
        (key(h), a.id, best.map(|(_, x, y)| (x, y)))
    };
    let (hkey, hroom, cell) = hop;
    let Some((x, y)) = cell else {
        walk.seen.insert(hkey);
        walk.blocked.insert(hkey);
        return PokeResult::Pending;
    };
    let placed = sim.lend(|a| {
        a.with(game, |g, v| {
            place_unit(PathCtx::of(v, g), player, Some(hroom), x, y, false, false)
        })
    });
    // A refused hop, or one that left the player outside H, marks H seen
    // (rule 3.3).
    let landed = placed
        && game
            .lists
            .unit(player)
            .and_then(|e| e.room())
            .is_some_and(|r| r == hroom);
    if !landed {
        walk.blocked.insert(hkey);
        walk.seen.insert(hkey);
    }
    PokeResult::Pending
}

/// Item creation `0x00558D90` with spawn mode 3 (ground), init flags 1,
/// in `room`, then the path part of `SUNIT_Add` at (x, y) (the drop
/// path's placement, `wiring::economy::death`).
#[allow(clippy::too_many_arguments)]
fn create_ground_item<X: crate::wiring::action::Pending>(
    game: &mut Game,
    v: &mut crate::wiring::action::View<'_, X>,
    tables: &ItemTables,
    room: RoomId,
    index: usize,
    quality: Option<u8>,
    ilvl: Option<u8>,
    x: i32,
    y: i32,
) -> Option<UnitId> {
    let h = &mut *v.h;
    let mut fields = GameFields::from_action(
        h.game_seed,
        &h.ai_info,
        v.data.expansion,
        std::mem::take(&mut h.uniques),
    );
    let mut items = std::mem::take(&mut h.items);
    let made = {
        let mut econ = Economy {
            game: &mut *game,
            units: &mut *v.units,
            stats: &mut *v.stats,
            data: v.data,
            hooks: &mut *v.h,
            fields: &mut fields,
            tables,
            items: &mut items,
        };
        let mut rq = ItemRequest {
            ilvl: ilvl.map_or(1, i32::from),
            item: index as i32,
            quality: quality.unwrap_or(0),
            format: ItemGame::item_format(&*econ.fields),
            ..ItemRequest::default()
        };
        econ.create_item(
            &mut rq,
            false,
            ItemSpawn {
                room: Some(room),
                mode: 3,
                init_flags: 1,
            },
        )
        .ok()
    };
    let h = &mut *v.h;
    h.items = items;
    h.game_seed = fields.seed;
    h.uniques = std::mem::take(&mut fields.uniques);
    let u = made?;
    v.path_place(game, u, x, y);
    Some(u)
}

/// The calls of a `spawn` step (`scenario.md` §3.1 rule 2) at (x, y) in
/// the active room of `act` that holds the point; the unit the first
/// call returned.
#[allow(clippy::too_many_arguments)]
pub fn spawn_monster<X: WorldPending>(
    game: &mut Game,
    ev: &mut WorldSim<X>,
    act: u8,
    class: u32,
    x: i32,
    y: i32,
    kind: SpawnKind,
    umods: &[u8],
) -> Option<UnitId> {
    let room = game.lists.active_rooms(act).into_iter().find(|&r| {
        ev.action
            .sys
            .hooks
            .drlg
            .subtiles(game, r)
            .is_some_and(|s| x >= s.x && x < s.x + s.w && y >= s.y && y < s.y + s.h)
    })?;
    let class = i32::try_from(class).ok()?;
    let unit = match kind {
        SpawnKind::Normal => ev.population(game, |cx| {
            placement::place_at(cx, room, None, x, y, class, 1, -1, 0).unit()
        })?,
        SpawnKind::RandomBoss => ev.population(game, |cx| {
            let b = pop_spawn::random_boss(cx, room, None, class, true, x, y, false)?;
            pop_spawn::champion_minions(cx, None, b, class);
            Some(b)
        })?,
        SpawnKind::Champion => {
            let b = ev.population(game, |cx| {
                pop_spawn::boss_spawn(cx, room, None, x, y, None, class, false)
            })?;
            let umod = *umods.first()?;
            ev.init(game, |cx, h| init::champion_pack_member(cx, h, b, umod));
            ev.population(game, |cx| pop_spawn::champion_minions(cx, None, b, class));
            b
        }
        SpawnKind::Unique => {
            let b = ev.population(game, |cx| {
                pop_spawn::boss_spawn(cx, room, None, x, y, None, class, false)
            })?;
            ev.init(game, |_, h| {
                for &u in umods {
                    h.monsters().entry(b).push_umod(u);
                }
            });
            ev.population(game, |cx| {
                pop_spawn::boss_minions_and_init(cx, b, 3, 6, None)
            });
            b
        }
    };
    Some(unit)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One line of every directive, with every optional argument and
    /// every reference form.
    const ALL: &[&str] = &[
        "goto unit 1:156",
        "goto preset 107 2:376",
        "object 119 100 200",
        "object 119 @x+5 @y-3 mode 1",
        "superunique 3 @x @y+10",
        "missile 7 @x @y @x+10 @y",
        "missile 7 100 200 110 200 skill 36 1 owner @1:19#2",
        "missile 7 100 200 110 200 owner 1/42",
        "seed-game 305419896 666",
        "seed-unit @1:19 1 2",
        "seed-unit @wp#1 1 2",
        "time 5 1024",
        "pos @player @x+3 @y",
        "warp 3",
        "warp 3 tile 2",
        "item hp1 @x @y",
        "item rin @x @y quality rare ilvl 85",
        "stat @player 0 0 -5",
        "stat @0 14 0 500000",
        "state @player 1 on",
        "state 1/9 2 off",
        "freeze 3",
    ];

    // Covers: specs/tools/poke.md §1 r1, §1 r2, §3 r2
    #[test]
    fn every_directive_round_trips() {
        let mut seen = BTreeSet::new();
        for line in ALL {
            let d = parse_directive_text(line).unwrap_or_else(|e| panic!("{line}: {e}"));
            seen.insert(d.keyword());
            let text = d.to_string();
            assert_eq!(&text, line, "canonical");
            assert_eq!(parse_directive_text(&text).unwrap(), d);
        }
        assert_eq!(seen.len(), KEYWORDS.len());
        // Hex and reordered optional arguments come back canonical.
        let d =
            parse_directive_text("missile 0x7 0x64 200 110 200 owner @player skill 36 1").unwrap();
        assert_eq!(
            d.to_string(),
            "missile 7 100 200 110 200 skill 36 1 owner @player"
        );
        assert_eq!(
            parse_directive_text("item rin 1 2 ilvl 5 quality magic")
                .unwrap()
                .to_string(),
            "item rin 1 2 quality magic ilvl 5"
        );
        // `goto`: the type defaults to 1 and is written out.
        assert_eq!(
            parse_directive_text("goto unit 5").unwrap().to_string(),
            "goto unit 1:5"
        );
        assert_eq!(
            parse_directive_text("goto preset 107 376")
                .unwrap()
                .to_string(),
            "goto preset 107 1:376"
        );
    }

    // Covers: specs/tools/poke.md §2 r5
    #[test]
    fn malformed_directives_name_the_problem() {
        for (line, needle) in [
            ("teleport 1 2", "unknown directive"),
            ("object 1 2", "needs 3"),
            ("superunique 1 2 3 4", "takes 3"),
            ("time 6 0", "period 6"),
            ("time 1", "takes 2"),
            ("object 1 2 3 mode 9", "object mode 9"),
            ("object 1 2 3 mode 1 mode 2", "given twice"),
            ("object 1 2 3 bogus 1", "unexpected"),
            ("seed-game 0x100000000 1", "32 bits"),
            ("seed-unit @x 1 2", "a unit is"),
            ("pos @player @z 2", "a position is"),
            ("pos @player @x+0 2", "zero offset"),
            ("pos 6/1 1 2", "unit type 6"),
            ("item HP1 1 2", "item code"),
            ("item hp1 1 2 quality best", "unknown quality"),
            ("item hp1 1 2 ilvl 100", "ilvl 100"),
            ("stat @player 0 0 2147483648", "does not fit i32"),
            ("state @player 1 maybe", "on or off"),
            ("warp 3 tile", "warp <level>"),
            ("missile 1 2 3 4 5 skill 1", "unexpected"),
            ("freeze 3601", "seconds"),
            ("goto unit 3:5", "goto unit type"),
            ("goto preset 2", "goto unit"),
            ("goto here 5", "goto unit"),
            ("goto unit 1:x", "bad number"),
            ("", "empty"),
        ] {
            let e = parse_directive_text(line).unwrap_err();
            assert!(e.contains(needle), "{line:?}: {e}");
        }
    }

    // Covers: specs/tools/poke.md §2 r1, §2 r2, §2 r3, §2 r5, §1 r3
    #[test]
    fn poke_files_parse_strictly_and_round_trip() {
        let text = "# a comment\n\npoke 1\nat 1 spawn 19 @x+3 @y normal\nat 1 seed-unit @1:19 1 2 # why\nat 4 time 0x2 10\nat 4 spawn 5 1 2 unique umod 3 7\n";
        let f = PokeFile::parse(text).unwrap();
        assert_eq!(f.lines.len(), 4);
        let canon = f.to_text();
        assert_eq!(
            canon,
            "poke 1\nat 1 spawn 19 @x+3 @y normal\nat 1 seed-unit @1:19 1 2\nat 4 time 2 10\nat 4 spawn 5 1 2 unique umod 3 7\n"
        );
        assert_eq!(PokeFile::parse(&canon).unwrap(), f);
        for (bad, line, needle) in [
            ("poke 2\n", 1, "version 2"),
            ("scenario 1\n", 1, "`poke 1`"),
            (
                "poke 1\nat 5 time 1 1\nat 4 time 1 1\n",
                3,
                "after a line at tick 5",
            ),
            ("poke 1\nat 1 bogus\n", 2, "unknown directive"),
            ("poke 1\ntime 1 1\n", 2, "`at <tick>"),
            ("poke 1\nat 1 time 1\n", 2, "takes 2"),
            ("poke 1\nat 1 time 1 2 3\n", 2, "takes 2"),
            (
                "poke 1\nat 1 spawn 19 1 2 champion\n",
                2,
                "one for champion",
            ),
            ("poke 1\nat 1000001 freeze 1\n", 2, "tick"),
            ("", 0, "empty"),
        ] {
            let e = PokeFile::parse(bad).unwrap_err();
            assert_eq!(e.line, line, "{bad:?}: {e}");
            assert!(e.message.contains(needle), "{bad:?}: {e}");
        }
    }

    #[test]
    fn results_name_their_code() {
        assert_eq!(PokeResult::Ok(Some(3)).code(), "ok");
        assert_eq!(PokeResult::Ok(Some(3)).guid(), Some(3));
        assert_eq!(PokeResult::Failed.code(), "failed");
        assert_eq!(PokeResult::Unresolved("@1".into()).code(), "unresolved");
        assert_eq!(PokeResult::Gap("x".into()).guid(), None);
    }
}
