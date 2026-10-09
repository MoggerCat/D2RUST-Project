// Spec: specs/tools/poke.md §5 rule 2 (the `d2-client play --poke` caller)
//! `play --poke "<f> <directive> <args>..."` and `play --poke-file FILE`:
//! pokes (`d2_sim::poke`) applied on the server thread between frames,
//! through the link's before-pump hook ([`ThreadLink::set_before_pump`]).
//!
//! When a poke runs:
//!
//! - `--poke "<f> ..."`: `f` is the absolute server frame (`Game.frame`
//!   after the tick): the poke runs once frame f − 1 has run, before
//!   frame f's drain and tick (`poke.md` §2 rule 4's point).
//! - `--poke-file`: the file's ticks are relative (`poke.md` §2 rule 4):
//!   tick 0 is the first frame after the one in which the local client
//!   reached state 4 (in game), so tick t runs when `Game.frame` = F0 + t,
//!   F0 being that frame (seen by the hook before the next pump).
//!
//! References are resolved by `d2_sim::poke::apply` on the state the
//! hook sees (every form of `poke.md` §1 rule 1; `@wp` from the waypoint
//! table of the game). Each result is printed to stderr. Without either
//! flag nothing is installed.

use std::collections::BTreeSet;

use d2_sim::poke::{self, Directive, GotoTarget, GotoWalk, PokeFile, PokeOp};
use d2_sim::units::lists::client_state;

use super::server_thread::{ThreadLink, ThreadStopped};
use super::single_player::{local_player, Link, Sim};
use crate::bridge::link::LOCAL_CLIENT;

/// When an entry runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum When {
    /// Absolute server frame f: runs when `Game.frame` ≥ f − 1.
    Frame(i32),
    /// Relative tick t of a poke file: runs when `Game.frame` ≥ F0 + t.
    Tick(u32),
}

/// One scheduled poke.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub when: When,
    pub op: PokeOp,
}

/// Parses one `--poke` value: `<f> <directive> <args>...` (or `<f> spawn
/// ...`).
pub fn parse_poke_arg(s: &str) -> Result<Entry, String> {
    let toks: Vec<&str> = s.split_ascii_whitespace().collect();
    let Some((f, rest)) = toks.split_first() else {
        return Err("--poke \"<frame> <directive> <args>...\"".into());
    };
    let f: i32 = f
        .parse()
        .ok()
        .filter(|&f| f >= 1)
        .ok_or_else(|| format!("--poke: frame {f:?}: a server frame ≥ 1"))?;
    let op = poke::parse_op(rest).map_err(|e| format!("--poke {s:?}: {e}"))?;
    Ok(Entry {
        when: When::Frame(f),
        op,
    })
}

/// The entries of a poke file (`poke.md` §2).
pub fn parse_poke_file(text: &str) -> Result<Vec<Entry>, String> {
    let f = PokeFile::parse(text).map_err(|e| e.to_string())?;
    Ok(f.lines
        .into_iter()
        .map(|l| Entry {
            when: When::Tick(l.tick),
            op: l.op,
        })
        .collect())
}

/// The frame after which `when` is due (`Game.frame` must have reached
/// it), `None` while the anchor F0 is unknown.
pub fn due_after(when: When, anchor: Option<i32>) -> Option<i32> {
    match when {
        When::Frame(f) => Some(f - 1),
        When::Tick(t) => anchor.map(|a| a.saturating_add(i32::try_from(t).unwrap_or(i32::MAX))),
    }
}

/// The pending pokes of a game.
#[derive(Debug, Default)]
pub struct Schedule {
    pending: Vec<Entry>,
    /// `goto` walks still stepping (`poke.md` §6), with their state.
    walking: Vec<(Entry, GotoWalk)>,
    /// F0: the frame in which the local client was first seen in game.
    anchor: Option<i32>,
}

impl Schedule {
    pub fn new(entries: Vec<Entry>) -> Self {
        Self {
            pending: entries,
            walking: Vec::new(),
            anchor: None,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.pending.is_empty() && self.walking.is_empty()
    }

    /// Runs every due entry in order on `s` and prints its result.
    pub fn run_due(&mut self, s: &mut Sim) {
        if self.is_empty() {
            return;
        }
        let frame = s.game.frame;
        if self.anchor.is_none()
            && s.game
                .lists
                .client(d2_sim::units::ClientId(LOCAL_CLIENT))
                .is_some_and(|c| c.state == client_state::IN_GAME)
        {
            self.anchor = Some(frame);
        }
        if local_player(s).is_none() {
            return;
        }
        let anchor = self.anchor;
        let (due, later): (Vec<Entry>, Vec<Entry>) = std::mem::take(&mut self.pending)
            .into_iter()
            .partition(|e| due_after(e.when, anchor).is_some_and(|f| frame >= f));
        self.pending = later;
        let walks = std::mem::take(&mut self.walking);
        let due = walks
            .into_iter()
            .map(|(e, w)| (e, Some(w)))
            .chain(due.into_iter().map(|e| (e, None)));
        for (e, walk) in due {
            let r = match (&e.op, walk) {
                (PokeOp::Directive(Directive::Goto(t)), w) => {
                    let (r, w) = goto_now(s, *t, w.unwrap_or_default());
                    if r == poke::PokeResult::Pending {
                        self.walking.push((e, w));
                        continue;
                    }
                    r
                }
                _ => apply_now(s, &e.op),
            };
            let late = match due_after(e.when, anchor) {
                Some(f) if frame > f => format!(" (late: due after frame {f})"),
                _ => String::new(),
            };
            eprintln!(
                "poke: after frame {frame}{late}: {}: {}{}",
                e.op,
                r.code(),
                match &r {
                    poke::PokeResult::Ok(Some(g)) => format!(" guid {g}"),
                    poke::PokeResult::Unresolved(u) => format!(" {u}"),
                    poke::PokeResult::Gap(why) => format!(" ({why})"),
                    _ => String::new(),
                }
            );
        }
    }
}

/// The `objects` rows with operate function 23 (`@wp`, `world/waypoints.md` §5).
pub(crate) fn waypoint_classes(s: &Sim) -> BTreeSet<u32> {
    s.world
        .action
        .waypoints
        .as_ref()
        .map(|w| {
            w.objects
                .iter()
                .enumerate()
                .filter(|(_, o)| o.operate_fn == 23)
                .map(|(i, _)| i as u32)
                .collect()
        })
        .unwrap_or_default()
}

/// Runs `op` on `s` now (`poke.md` §5): the local player is `@player`,
/// `@wp` and `item` use the game's own tables. No local player yet:
/// `unresolved @player`.
pub fn apply_now(s: &mut Sim, op: &PokeOp) -> poke::PokeResult {
    let Some((player, _)) = local_player(s) else {
        return poke::PokeResult::Unresolved("@player".into());
    };
    let waypoints = waypoint_classes(s);
    // A copy: the loan below takes the world's tables for its call.
    let tables = s.world.tables.clone();
    let env = poke::Env {
        player,
        waypoint_classes: &waypoints,
        items: Some(&tables),
    };
    // The quest parts are lent as in the tick and the 0x13 / waypoint
    // handlers, so a quest object a directive creates (a `warp` that
    // builds an act) runs its init (`quests-act2-2.md` §2 item 1).
    let game = &mut s.game;
    s.world
        .lend_quests(&mut s.events, |_, ev| poke::apply_op(game, ev, &env, op))
}

/// One step of a `goto` walk (`poke.md` §6) on `s`, with the walk's
/// state; no local player: `unresolved @player`.
pub fn goto_now(s: &mut Sim, t: GotoTarget, mut walk: GotoWalk) -> (poke::PokeResult, GotoWalk) {
    let Some((player, _)) = local_player(s) else {
        return (poke::PokeResult::Unresolved("@player".into()), walk);
    };
    let env = poke::Env::new(player);
    let r = poke::goto_step(&mut s.game, &mut s.events, &env, &t, &mut walk);
    (r, walk)
}

/// The keyword a `poke` record names (`d`): the directive's, or `spawn`.
pub fn op_keyword(op: &PokeOp) -> &'static str {
    match op {
        PokeOp::Directive(d) => d.keyword(),
        PokeOp::Spawn(_) => "spawn",
    }
}

/// The `poke` record of an absolute `--poke` result (`poke.md` §2 r6,
/// §3 r3; the fields `poke.py` writes on 1.14d): `f` the frame the poke
/// precedes, `frame` = f − 1, `i` its index among the pokes run at that
/// point, `d`, `r`, `guid` when `ok` created a unit, `note` for
/// `unresolved` / `gap`, `src` the canonical directive.
pub fn record_line(f: i32, i: usize, op: &PokeOp, r: &poke::PokeResult) -> String {
    record_line_steps(f, i, op, r, None)
}

/// [`record_line`] with a `goto`'s `steps` (`poke.md` §6 rule 4).
pub fn record_line_steps(
    f: i32,
    i: usize,
    op: &PokeOp,
    r: &poke::PokeResult,
    steps: Option<u32>,
) -> String {
    use d2_sim::debug::state::json_string;
    let mut o = format!(
        "{{\"k\":\"poke\",\"f\":{f},\"frame\":{},\"i\":{i},\"d\":{},\"r\":{}",
        f - 1,
        json_string(op_keyword(op)),
        json_string(r.code())
    );
    match r {
        poke::PokeResult::Ok(Some(g)) => o.push_str(&format!(",\"guid\":{g}")),
        poke::PokeResult::Unresolved(n) | poke::PokeResult::Gap(n) => {
            o.push_str(&format!(",\"note\":{}", json_string(n)));
        }
        _ => {}
    }
    if let Some(n) = steps {
        o.push_str(&format!(",\"steps\":{n}"));
    }
    o.push_str(&format!(",\"src\":{}}}", json_string(&op.to_string())));
    o
}

/// The server thread runs the poke between two frames (`poke.md` §2 r6).
impl<C: d2_server::seams::Clock + Send + 'static> crate::bridge::poke::PokeTarget
    for ThreadLink<Link<C>>
{
    type Error = ThreadStopped;
    fn poke(&mut self, op: &PokeOp) -> Result<poke::PokeResult, Self::Error> {
        let op = op.clone();
        self.with(move |l| apply_now(&mut l.host_mut().game, &op))
    }
    fn goto_step(
        &mut self,
        target: GotoTarget,
        walk: GotoWalk,
    ) -> Result<(poke::PokeResult, GotoWalk), Self::Error> {
        self.with(move |l| goto_now(&mut l.host_mut().game, target, walk))
    }
}

/// Installs `entries` on the game's server thread (no-op when empty).
pub fn install<C: d2_server::seams::Clock + Send + 'static>(
    link: &mut ThreadLink<Link<C>>,
    entries: Vec<Entry>,
) -> Result<(), ThreadStopped> {
    if entries.is_empty() {
        return Ok(());
    }
    let mut schedule = Schedule::new(entries);
    link.set_before_pump(move |l: &mut Link<C>| schedule.run_due(&mut l.host_mut().game))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Covers: specs/tools/poke.md §5 r2
    #[test]
    fn poke_args_and_files_parse_and_schedule() {
        let e = parse_poke_arg("40 time 2 0").unwrap();
        assert_eq!(e.when, When::Frame(40));
        assert_eq!(e.op.to_string(), "time 2 0");
        let e = parse_poke_arg("3 spawn 19 @x+4 @y normal").unwrap();
        assert!(matches!(e.op, PokeOp::Spawn(_)));
        assert!(parse_poke_arg("0 time 2 0").is_err());
        assert!(parse_poke_arg("x time 2 0").is_err());
        assert!(parse_poke_arg("5 time 9 0")
            .unwrap_err()
            .contains("period 9"));
        assert!(parse_poke_arg("").is_err());
        let f = parse_poke_file("poke 1\nat 0 seed-game 1 2\nat 7 freeze 1\n").unwrap();
        assert_eq!(f[1].when, When::Tick(7));
        assert!(parse_poke_file("poke 2\n").is_err());
        // Absolute frame f: after frame f − 1; relative tick t: after F0 + t.
        assert_eq!(due_after(When::Frame(40), None), Some(39));
        assert_eq!(due_after(When::Tick(7), None), None);
        assert_eq!(due_after(When::Tick(7), Some(100)), Some(107));
        assert!(Schedule::new(Vec::new()).is_empty());
    }
}
