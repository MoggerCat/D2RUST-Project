// Spec: specs/tools/poke.md §6 (`goto`)
//! `goto` on d2rs: a target in the player's active rooms is reached in
//! one step (the player placed by the free-point search next to it, the
//! target's GUID as the result); a target that exists nowhere ends
//! `failed` after the walk has seen every room it can reach, with
//! `pending` steps between ticks while there are rooms left.

use std::path::PathBuf;
use std::sync::OnceLock;

use d2_sim::poke::{self, Directive, Env, GotoWalk, PokeResult, GOTO_MAX_STEPS};
use d2_sim::units::UnitType;
use test_fixtures::game::{ActCreation, GameData};
use test_fixtures::host::{cheb, Session, Setup};
use test_fixtures::{install, synth};

fn synthetic() -> &'static GameData {
    static D: OnceLock<GameData> = OnceLock::new();
    D.get_or_init(|| {
        let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("poke-goto-{}", std::process::id()));
        let i = install::build(&dir, &synth::synthetic()).unwrap_or_else(|e| panic!("{e}"));
        GameData::from_install(&i).unwrap_or_else(|e| panic!("{e}"))
    })
}

fn session() -> Session {
    Session::new(
        synthetic(),
        &Setup {
            creation: ActCreation::TownOnly,
            init_seed: 0x1234_5678,
            town: 1,
            game_seed: 1234,
            class: 3,
            known_waypoints: Vec::new(),
        },
    )
}

fn line(s: &mut Session, text: &str) -> PokeResult {
    let player = s.player;
    let sim = s.sim();
    poke::apply_line(&mut sim.game, &mut sim.events, &Env::new(player), text)
        .unwrap_or_else(|e| panic!("{text}: {e}"))
}

/// Steps `text` (a `goto`) once per frame until it is not pending.
fn walk(s: &mut Session, text: &str) -> (PokeResult, GotoWalk) {
    let Directive::Goto(t) = poke::parse_directive_text(text).unwrap() else {
        panic!("{text}: not a goto");
    };
    let mut w = GotoWalk::default();
    loop {
        let player = s.player;
        let r = {
            let sim = s.sim();
            poke::goto_step(
                &mut sim.game,
                &mut sim.events,
                &Env::new(player),
                &t,
                &mut w,
            )
        };
        if r != PokeResult::Pending {
            return (r, w);
        }
        assert!(w.steps <= GOTO_MAX_STEPS, "a pending step past the limit");
        s.frame();
    }
}

// Covers: specs/tools/poke.md §6 r3.2
#[test]
fn goto_a_unit_in_the_active_rooms_lands_next_to_it() {
    let mut s = session();
    let PokeResult::Ok(Some(guid)) = line(&mut s, "object 1 @x+8 @y") else {
        panic!("object poke");
    };
    let obj = s
        .sim()
        .game
        .lists
        .find_unit(UnitType::Object, guid)
        .expect("the object");
    let at = s.sim().events.action.hooks().path_position(obj);
    // The first unit of the class, ascending GUID, in the player's level.
    let first = {
        let sim = s.sim();
        let mut v: Vec<u32> = sim
            .game
            .lists
            .units_of_type(UnitType::Object)
            .into_iter()
            .filter(|&u| {
                sim.events
                    .action
                    .sys
                    .units
                    .get(u)
                    .is_some_and(|r| r.class == 1)
            })
            .filter_map(|u| sim.game.lists.unit(u).map(|e| e.guid))
            .collect();
        v.sort_unstable();
        v[0]
    };
    let (r, w) = walk(&mut s, "goto unit 2:1");
    assert_eq!(r, PokeResult::Ok(Some(first)));
    assert_eq!(w.steps, 1);
    if first == guid {
        let p = s.pos();
        assert!(cheb(p, at) <= 3, "player at {p:?}, object at {at:?}");
    }
    assert_eq!(s.sim().events.errors(), Vec::<String>::new());
}

// Covers: specs/tools/poke.md §6 r3.3, r3.4
#[test]
fn goto_a_missing_target_explores_then_fails() {
    let mut s = session();
    let (r, w) = walk(&mut s, "goto unit 2:4000");
    assert_eq!(r, PokeResult::Failed);
    assert!(
        w.steps >= 1 && w.steps <= GOTO_MAX_STEPS,
        "{} steps",
        w.steps
    );
    assert!(!w.seen.is_empty());
    // Every room of the town reachable through the near arrays was seen:
    // one more walk from the end sees nothing new.
    let seen = w.seen.len();
    let (r2, w2) = walk(&mut s, "goto unit 2:4000");
    assert_eq!(r2, PokeResult::Failed);
    assert!(w2.seen.len() <= seen, "{} > {seen}", w2.seen.len());
    // A one-call `apply` is one step with a fresh walk.
    let r3 = line(&mut s, "goto unit 2:4000");
    assert!(
        matches!(r3, PokeResult::Failed | PokeResult::Pending),
        "{r3:?}"
    );
}
