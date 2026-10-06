// Spec: specs/monsters/init.md (monster data: nTypeFlag, nMonUmod, wBossHcIdx); specs/drlg/levels.md §10 (the anchors)
//! The query: what a seed's level must hold, parsed from flags, and the
//! check of one seed's [`LevelView`] against it. Nothing here decides
//! game behaviour: the view is what the real DRLG and population built.

use std::fmt;

use anyhow::{anyhow, bail, Context, Result};

use crate::world::{LevelView, Monster};

/// The kind of a monster, from its type flags (`init.md`, nTypeFlag):
/// superunique, then unique, champion, minion; else normal.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Kind {
    Normal,
    Minion,
    Champion,
    Unique,
    SuperUnique,
}

impl Kind {
    pub fn from_flags(flags: u16) -> Self {
        use d2_sim::monsters::init::type_flag as f;
        if flags & f::SUPERUNIQUE != 0 {
            Kind::SuperUnique
        } else if flags & f::UNIQUE != 0 {
            Kind::Unique
        } else if flags & f::CHAMPION != 0 {
            Kind::Champion
        } else if flags & f::MINION != 0 {
            Kind::Minion
        } else {
            Kind::Normal
        }
    }

    fn parse(s: &str) -> Result<Self> {
        Ok(match s {
            "normal" => Kind::Normal,
            "minion" => Kind::Minion,
            "champion" => Kind::Champion,
            "unique" => Kind::Unique,
            "superunique" | "su" => Kind::SuperUnique,
            _ => bail!("unknown kind {s:?} (normal, minion, champion, unique, superunique)"),
        })
    }
}

impl fmt::Display for Kind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Kind::Normal => "normal",
            Kind::Minion => "minion",
            Kind::Champion => "champion",
            Kind::Unique => "unique",
            Kind::SuperUnique => "superunique",
        })
    }
}

/// A monstats row: by number, or by `Id` when the install has the text
/// table (resolved by [`Query::resolve`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ClassSel {
    Row(u32),
    Name(String),
}

/// One wanted group of monsters: at least `count` monsters of the level
/// with every condition given.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Want {
    pub class: Option<ClassSel>,
    pub kind: Option<Kind>,
    /// Superunique row (`wBossHcIdx`).
    pub superunique: Option<u16>,
    /// Boss modifiers (monumod rows) the monster must all have.
    pub umods: Vec<u8>,
    pub count: u32,
}

impl Want {
    /// `key=value` pairs joined by commas: `class=<row|Id>`,
    /// `kind=<normal|minion|champion|unique|superunique>`, `su=<row>`,
    /// `umod=<id>[+<id>…]`, `count=<n>` (default 1).
    pub fn parse(spec: &str) -> Result<Self> {
        let mut w = Want {
            class: None,
            kind: None,
            superunique: None,
            umods: Vec::new(),
            count: 1,
        };
        for part in spec.split(',').filter(|p| !p.is_empty()) {
            let (k, v) = part
                .split_once('=')
                .ok_or_else(|| anyhow!("monster spec {spec:?}: {part:?} is not key=value"))?;
            match k {
                "class" => {
                    w.class = Some(match v.parse::<u32>() {
                        Ok(r) => ClassSel::Row(r),
                        Err(_) => ClassSel::Name(v.to_owned()),
                    })
                }
                "kind" => w.kind = Some(Kind::parse(v)?),
                "su" => {
                    w.superunique = Some(v.parse().with_context(|| format!("su={v}"))?);
                    w.kind.get_or_insert(Kind::SuperUnique);
                }
                "umod" | "umods" => {
                    for u in v.split('+') {
                        w.umods
                            .push(u.parse().with_context(|| format!("umod {u}"))?);
                    }
                }
                "count" => w.count = v.parse().with_context(|| format!("count={v}"))?,
                _ => bail!("monster spec {spec:?}: unknown key {k:?}"),
            }
        }
        Ok(w)
    }

    fn matches(&self, m: &Monster) -> bool {
        let class_ok = match &self.class {
            None => true,
            Some(ClassSel::Row(r)) => m.class == *r,
            Some(ClassSel::Name(_)) => false, // resolved before the search
        };
        class_ok
            && self.kind.is_none_or(|k| k == m.kind)
            && self.superunique.is_none_or(|s| m.superunique == Some(s))
            && self.umods.iter().all(|u| m.umods.contains(u))
    }
}

/// What a distance is measured from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Anchor {
    /// The level's entrance ([`crate::world::Anchors::entrance`]).
    Entrance,
    /// The level's waypoint (`levels.md` §10.4).
    Waypoint,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Near {
    pub anchor: Anchor,
    /// Euclidean distance in tiles (5 sub-tiles each).
    pub tiles: u32,
}

impl Near {
    /// `entrance:<tiles>` or `waypoint:<tiles>`.
    pub fn parse(s: &str) -> Result<Self> {
        let (a, n) = s
            .split_once(':')
            .ok_or_else(|| anyhow!("--near {s:?}: want entrance:<tiles> or waypoint:<tiles>"))?;
        let anchor = match a {
            "entrance" => Anchor::Entrance,
            "waypoint" | "wp" => Anchor::Waypoint,
            _ => bail!("--near {s:?}: unknown anchor {a:?}"),
        };
        Ok(Near {
            anchor,
            tiles: n.parse().with_context(|| format!("--near {s}"))?,
        })
    }
}

/// How the two seeds of a candidate are chosen from the searched value
/// `s`: the game seed (`rng.md` §5.2, the scenario's `seed`) and the
/// DRLG map seed (`rng.md` §5.4, the scenario's `init_seed`, which
/// defaults to `seed`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SeedPlan {
    /// `None`: the map seed is `s`.
    pub init_seed: Option<u32>,
    /// `None`: the game seed is `s`.
    pub game_seed: Option<u32>,
}

impl SeedPlan {
    /// (map seed, game seed) of the searched value `s`.
    pub fn seeds(&self, s: u32) -> (u32, u32) {
        (self.init_seed.unwrap_or(s), self.game_seed.unwrap_or(s))
    }
}

/// A whole query.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Query {
    /// levels.txt `Id`.
    pub level: u32,
    pub difficulty: u8,
    /// The level the entrance leads from (`--from`).
    pub from_level: Option<u32>,
    pub wants: Vec<Want>,
    pub near: Option<Near>,
    /// Preset units that must be in the level: (unit type, class).
    pub presets: Vec<(u32, u32)>,
    /// Searched values, inclusive.
    pub first: u32,
    pub last: u32,
    /// Stop after this many matches (the lowest seeds).
    pub limit: Option<usize>,
    pub plan: SeedPlan,
    pub threads: usize,
}

impl Default for Query {
    fn default() -> Self {
        Query {
            level: 0,
            difficulty: 0,
            from_level: None,
            wants: Vec::new(),
            near: None,
            presets: Vec::new(),
            first: 1,
            last: 1000,
            limit: None,
            plan: SeedPlan {
                init_seed: None,
                game_seed: None,
            },
            threads: 1,
        }
    }
}

pub const USAGE: &str = "\
seed-finder: search game seeds for a level and monster situation.

  seed-finder --level <id> [options]      (reads D2_GAME_DIR)

  --level <id>            levels.txt Id (required)
  --difficulty <0|1|2>    default 0
  --monster <spec>        repeatable; comma-separated key=value:
                            class=<monstats row|Id>  kind=<normal|minion|champion|unique|superunique>
                            su=<superuniques row>    umod=<id>[+<id>...]   count=<n> (default 1)
  --near <anchor>:<tiles> every wanted monster within <tiles> of the anchor
                            (entrance | waypoint); Euclidean, tiles of 5 sub-tiles
  --from <level id>       the level the entrance leads from
  --preset <type>:<class> a preset unit (1 monster, 2 object, ...) must be in the level
  --seeds <a>-<b>         searched values, inclusive (default 1-1000)
  --limit <n>             stop after the n lowest matching seeds
  --init-seed <u32>       fix the DRLG map seed (default: the searched value)
  --game-seed <u32>       fix the game seed (default: the searched value)
  --threads <n>           worker threads (default: available parallelism)
  --town-only             act creation without the act placer (synthetic tables only)
";

/// Flags parsed: the query and whether the act placer is skipped.
pub struct Args {
    pub query: Query,
    pub town_only: bool,
}

fn num<T: std::str::FromStr>(flag: &str, v: &str) -> Result<T>
where
    T::Err: std::error::Error + Send + Sync + 'static,
{
    v.parse::<T>().with_context(|| format!("{flag} {v}"))
}

/// Parses the command line (without the program name).
pub fn parse_args(args: &[String]) -> Result<Args> {
    let mut q = Query {
        threads: std::thread::available_parallelism().map_or(1, |n| n.get()),
        ..Query::default()
    };
    let mut level = None;
    let mut town_only = false;
    let mut it = args.iter();
    while let Some(flag) = it.next() {
        if flag == "--town-only" {
            town_only = true;
            continue;
        }
        if flag == "--help" || flag == "-h" {
            bail!("{USAGE}");
        }
        let v = it
            .next()
            .ok_or_else(|| anyhow!("{flag} needs a value\n\n{USAGE}"))?;
        match flag.as_str() {
            "--level" => level = Some(num(flag, v)?),
            "--difficulty" => {
                q.difficulty = num(flag, v)?;
                if q.difficulty > 2 {
                    bail!("--difficulty {v}: 0, 1 or 2");
                }
            }
            "--monster" => q.wants.push(Want::parse(v)?),
            "--near" => q.near = Some(Near::parse(v)?),
            "--from" => q.from_level = Some(num(flag, v)?),
            "--preset" => {
                let (t, c) = v
                    .split_once(':')
                    .ok_or_else(|| anyhow!("--preset {v:?}: want <type>:<class>"))?;
                q.presets.push((num(flag, t)?, num(flag, c)?));
            }
            "--seeds" => {
                let (a, b) = v
                    .split_once('-')
                    .ok_or_else(|| anyhow!("--seeds {v:?}: want <a>-<b>"))?;
                q.first = num(flag, a)?;
                q.last = num(flag, b)?;
                if q.first > q.last {
                    bail!("--seeds {v}: empty range");
                }
            }
            "--limit" => q.limit = Some(num(flag, v)?),
            "--init-seed" => q.plan.init_seed = Some(num(flag, v)?),
            "--game-seed" => q.plan.game_seed = Some(num(flag, v)?),
            "--threads" => q.threads = num::<usize>(flag, v)?.max(1),
            _ => bail!("unknown flag {flag}\n\n{USAGE}"),
        }
    }
    q.level = level.ok_or_else(|| anyhow!("--level is required\n\n{USAGE}"))?;
    if q.plan.init_seed.is_some() && q.plan.game_seed.is_some() {
        bail!("--init-seed and --game-seed together leave nothing to search");
    }
    Ok(Args {
        query: q,
        town_only,
    })
}

impl Query {
    /// Resolves class names against the monstats `Id` column (row order).
    pub fn resolve(&mut self, names: &[Option<String>]) -> Result<()> {
        for w in &mut self.wants {
            if let Some(ClassSel::Name(n)) = &w.class {
                let row = names
                    .iter()
                    .position(|x| x.as_deref().is_some_and(|x| x.eq_ignore_ascii_case(n)))
                    .ok_or_else(|| {
                        anyhow!("class {n:?}: no monstats Id of that name (use the row number)")
                    })?;
                w.class = Some(ClassSel::Row(row as u32));
            }
        }
        Ok(())
    }
}

/// One wanted group as found: the monsters (unit order) and their
/// distance from the anchor in tiles (rounded down) when one is asked.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Found {
    pub want: usize,
    pub monsters: Vec<(Monster, Option<u32>)>,
}

/// Why a seed does not match (the first failed condition).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Miss {
    NoAnchor(Anchor),
    NoPreset(u32, u32),
    Want(usize),
}

/// Integer square root (rounded down) of a non-negative value.
fn isqrt(v: i64) -> u32 {
    let (mut lo, mut hi) = (0i64, v.max(1));
    while lo < hi {
        let mid = (lo + hi + 1) / 2;
        if mid.saturating_mul(mid) <= v {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    lo as u32
}

/// Checks a level view: every preset present, every want met (within the
/// anchor distance when asked). Ok holds the groups found.
pub fn check(q: &Query, v: &LevelView) -> Result<Vec<Found>, Miss> {
    for &(t, c) in &q.presets {
        if !v.presets.iter().any(|p| p.unit_type == t && p.class == c) {
            return Err(Miss::NoPreset(t, c));
        }
    }
    let anchor = match q.near {
        None => None,
        Some(n) => {
            let at = match n.anchor {
                Anchor::Entrance => v.anchors.entrance,
                Anchor::Waypoint => v.anchors.waypoint,
            };
            Some((at.ok_or(Miss::NoAnchor(n.anchor))?, n.tiles))
        }
    };
    let mut out = Vec::new();
    for (i, w) in q.wants.iter().enumerate() {
        let mut got = Vec::new();
        for m in v.monsters.iter().filter(|m| w.matches(m)) {
            // Integer distance in sub-tiles squared against the radius.
            let d = anchor.map(|((ax, ay), t)| {
                let (dx, dy) = (i64::from(m.x - ax), i64::from(m.y - ay));
                (dx * dx + dy * dy, i64::from(t) * 5)
            });
            match d {
                Some((d2, r)) if d2 > r * r => continue,
                Some((d2, _)) => got.push((m.clone(), Some(isqrt(d2) / 5))),
                None => got.push((m.clone(), None)),
            }
        }
        if (got.len() as u32) < w.count {
            return Err(Miss::Want(i));
        }
        out.push(Found {
            want: i,
            monsters: got,
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(s: &str) -> Result<Args> {
        let a: Vec<String> = s.split_whitespace().map(str::to_owned).collect();
        parse_args(&a)
    }

    #[test]
    fn flags_parse() {
        let a = args(
            "--level 8 --difficulty 2 --monster class=ghoul1,kind=champion,count=3,umod=5+17 \
             --monster su=3 --near entrance:12 --from 2 --preset 1:250 --seeds 10-20 \
             --limit 4 --game-seed 99 --threads 2 --town-only",
        )
        .unwrap();
        let q = a.query;
        assert!(a.town_only);
        assert_eq!((q.level, q.difficulty, q.from_level), (8, 2, Some(2)));
        assert_eq!(
            q.wants[0],
            Want {
                class: Some(ClassSel::Name("ghoul1".into())),
                kind: Some(Kind::Champion),
                superunique: None,
                umods: vec![5, 17],
                count: 3,
            }
        );
        assert_eq!(q.wants[1].kind, Some(Kind::SuperUnique));
        assert_eq!(q.wants[1].superunique, Some(3));
        assert_eq!(
            q.near,
            Some(Near {
                anchor: Anchor::Entrance,
                tiles: 12
            })
        );
        assert_eq!(q.presets, [(1, 250)]);
        assert_eq!((q.first, q.last, q.limit, q.threads), (10, 20, Some(4), 2));
        assert_eq!(q.plan.seeds(15), (15, 99));
    }

    #[test]
    fn bad_flags_are_errors() {
        for bad in [
            "",
            "--level",
            "--level 1 --difficulty 3",
            "--level 1 --monster kind=boss",
            "--level 1 --monster colour=red",
            "--level 1 --near door:3",
            "--level 1 --seeds 9-3",
            "--level 1 --init-seed 1 --game-seed 2",
            "--level 1 --frobnicate 1",
        ] {
            assert!(args(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn names_resolve_by_row() {
        let mut q = args("--level 1 --monster class=Ghoul1").unwrap().query;
        q.resolve(&[Some("beast1".into()), Some("ghoul1".into())])
            .unwrap();
        assert_eq!(q.wants[0].class, Some(ClassSel::Row(1)));
        let mut q = args("--level 1 --monster class=imp").unwrap().query;
        assert!(q.resolve(&[None, None]).is_err());
    }

    #[test]
    fn kinds_from_type_flags() {
        use d2_sim::monsters::init::type_flag as f;
        assert_eq!(Kind::from_flags(0), Kind::Normal);
        assert_eq!(Kind::from_flags(f::MINION), Kind::Minion);
        assert_eq!(Kind::from_flags(f::CHAMPION), Kind::Champion);
        assert_eq!(Kind::from_flags(f::UNIQUE | f::BOSS), Kind::Unique);
        assert_eq!(
            Kind::from_flags(f::SUPERUNIQUE | f::UNIQUE | f::BOSS),
            Kind::SuperUnique
        );
    }

    #[test]
    fn isqrt_rounds_down() {
        for v in [0i64, 1, 2, 3, 4, 8, 9, 10, 99, 100, 101, 1 << 40] {
            let r = i64::from(isqrt(v));
            assert!(r * r <= v && (r + 1) * (r + 1) > v, "{v}");
        }
    }
}
