// Spec: specs/drlg/levels.md, specs/monsters/population.md, specs/monsters/init.md, specs/sim/rng.md §5.2, §5.4 (the paths the search runs; no rule of its own)
//! Seed finder for differential-testing scenarios: d2rs builds levels
//! and their monster populations from the seeds exactly as specified, so
//! it can search seeds for a wanted situation; the scenario then runs
//! that seed on the original and on d2rs.
//!
//! [`query`] parses and checks a query, [`world`] builds one seed's
//! level through the real DRLG and population path, [`search`] runs the
//! seeds over std threads and returns the matches sorted by seed.

pub mod query;
pub mod world;

use std::fmt::Write as _;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;

use anyhow::{bail, Result};

use query::{check, Found, Query};
use world::{build_level, FinderHost, LevelView, Prepared};

/// One searched value: its seeds and what its level held.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hit {
    /// The searched value.
    pub seed: u32,
    /// DRLG map seed (`rng.md` §5.4; scenario `init_seed`).
    pub init_seed: u32,
    /// Game seed value (`rng.md` §5.2; scenario `seed`).
    pub game_seed: u32,
    pub view: LevelView,
    pub found: Vec<Found>,
}

/// A search result: matches by seed, or the first seed the d2rs path
/// could not build (then the level is unsupported).
#[derive(Debug, PartialEq, Eq)]
pub struct Outcome {
    pub hits: Vec<Hit>,
    /// Values checked (a prefix of the range when `limit` stopped it).
    pub checked: u32,
    pub unsupported: Option<(u32, String)>,
}

/// Seeds per work item: the limit is applied after whole chunks, so the
/// hits kept are always the lowest seeds of the range.
const CHUNK: u32 = 16;

/// Rejects queries the host cannot answer: a champion or unique wanted
/// where room population places nothing.
pub fn supported<X: FinderHost>(q: &Query) -> Result<()> {
    if !X::ROOM_POPULATION {
        if let Some(k) = q
            .wants
            .iter()
            .filter_map(|w| w.kind)
            .find(|k| k.needs_room_population())
        {
            bail!(
                "unsupported query: kind {k} comes only from room population \
                 (population.md §3), whose DRLG coordinate lists (0x0061AD50) no \
                 spec provides yet; on this host only presets place monsters"
            );
        }
    }
    Ok(())
}

/// Checks one value.
pub fn check_seed<X: FinderHost>(
    p: &Prepared,
    q: &Query,
    seed: u32,
    host: X,
) -> Result<Option<Hit>> {
    let (init_seed, game_seed) = q.plan.seeds(seed);
    let view = build_level(
        p,
        q.level,
        q.difficulty,
        q.from_level,
        init_seed,
        game_seed,
        host,
    )?;
    Ok(check(q, &view).ok().map(|found| Hit {
        seed,
        init_seed,
        game_seed,
        view,
        found,
    }))
}

/// Runs the query over its seed range on `q.threads` threads; `host`
/// makes each game's seams. Output order does not depend on the threads.
pub fn search<X: FinderHost>(
    p: &Prepared,
    q: &Query,
    host: impl Fn() -> X + Sync,
) -> Result<Outcome> {
    supported::<X>(q)?;
    let span = u64::from(q.last) - u64::from(q.first) + 1;
    let chunks = span.div_ceil(u64::from(CHUNK)) as u32;
    let next = AtomicU32::new(0);
    // Per chunk: hits, or the first unsupported seed.
    type ChunkResult = (u32, Vec<Hit>, Option<(u32, String)>);
    let done: Mutex<Vec<ChunkResult>> = Mutex::new(Vec::new());
    // Lowest chunk index that already ends the search.
    let stop = AtomicU32::new(u32::MAX);
    std::thread::scope(|s| {
        for _ in 0..q.threads.max(1) {
            s.spawn(|| loop {
                let c = next.fetch_add(1, Ordering::SeqCst);
                if c >= chunks || c > stop.load(Ordering::SeqCst) {
                    break;
                }
                let lo = u64::from(q.first) + u64::from(c) * u64::from(CHUNK);
                let hi = (lo + u64::from(CHUNK) - 1).min(u64::from(q.last));
                let mut hits = Vec::new();
                let mut bad = None;
                for seed in lo..=hi {
                    match check_seed(p, q, seed as u32, host()) {
                        Ok(Some(h)) => hits.push(h),
                        Ok(None) => {}
                        Err(e) => {
                            bad = Some((seed as u32, format!("{e:#}")));
                            break;
                        }
                    }
                }
                let mut d = done.lock().expect("no panics hold the lock");
                d.push((c, hits, bad));
                // Stop once the finished prefix holds enough hits or an
                // unsupported seed.
                d.sort_by_key(|r| r.0);
                let mut n = 0;
                for (i, r) in d.iter().enumerate() {
                    if r.0 != i as u32 {
                        break;
                    }
                    n += r.1.len();
                    if r.2.is_some() || q.limit.is_some_and(|l| n >= l) {
                        stop.fetch_min(r.0, Ordering::SeqCst);
                        break;
                    }
                }
            });
        }
    });
    let mut d = done.into_inner().expect("no panics hold the lock");
    d.sort_by_key(|r| r.0);
    let mut out = Outcome {
        hits: Vec::new(),
        checked: 0,
        unsupported: None,
    };
    for (i, (c, hits, bad)) in d.into_iter().enumerate() {
        if c != i as u32 {
            break;
        }
        let lo = q.first + c * CHUNK;
        let hi = (u64::from(lo) + u64::from(CHUNK) - 1).min(u64::from(q.last)) as u32;
        out.hits.extend(hits);
        if let Some((seed, e)) = bad {
            out.checked += seed - lo + 1;
            out.unsupported = Some((seed, e));
            break;
        }
        out.checked += hi - lo + 1;
        if let Some(l) = q.limit {
            if out.hits.len() >= l {
                out.hits.truncate(l);
                out.checked = out
                    .hits
                    .last()
                    .map_or(out.checked, |h| h.seed - q.first + 1);
                break;
            }
        }
    }
    Ok(out)
}

/// The class label of a monstats row.
fn class_name(names: &[Option<String>], class: u32) -> String {
    match names.get(class as usize) {
        Some(Some(n)) => format!("{n} (#{class})"),
        _ => format!("#{class}"),
    }
}

/// A short description of a hit: seeds, rooms, anchors, matched units.
pub fn describe(h: &Hit, q: &Query, names: &[Option<String>]) -> String {
    let mut s = format!(
        "seed {} (init_seed {}, game seed {}): level {}, {} rooms, {} monsters",
        h.seed,
        h.init_seed,
        h.game_seed,
        h.view.level,
        h.view.rooms,
        h.view.monsters.len()
    );
    let a = h.view.anchors;
    if let Some((x, y)) = a.entrance {
        let _ = write!(s, ", entrance ({x}, {y})");
    }
    if let Some((x, y)) = a.waypoint {
        let _ = write!(s, ", waypoint ({x}, {y})");
    }
    for f in &h.found {
        let w = &q.wants[f.want];
        let _ = write!(s, "\n  want {} (count ≥ {}):", f.want + 1, w.count);
        for (m, d) in &f.monsters {
            let _ = write!(
                s,
                "\n    unit {} {} {} at ({}, {})",
                m.unit,
                m.kind,
                class_name(names, m.class),
                m.x,
                m.y
            );
            if let Some(su) = m.superunique {
                let _ = write!(s, " superunique #{su}");
            }
            if !m.umods.is_empty() {
                let _ = write!(s, " umods {:?}", m.umods);
            }
            if let Some(d) = d {
                let _ = write!(s, ", {d} tiles");
            }
        }
    }
    s
}
