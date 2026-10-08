// Spec: specs/sim/intents-events.md (§2.4 r3, r4), specs/sim/pathing.md (§9), specs/client/bridge.md (§3)
//! The play host's server core, headless, on the synthetic single-player
//! game behind the app's link (`single_player::start`): the play host
//! stages no `UnitFacts`, so the point and unit-target parse reads them
//! from the sim's own unit and path records (`SimGame::set_facts_source`,
//! `world_sim_facts`). Without them every C→S walk was `Invalid` and the
//! walk the client drew was only its own prediction.
//!
//! Test-local fill: the synthetic game has no `charstats` (walk velocity
//! 0), so the joined game's action tables get one row per class with
//! `WalkVelocity` 6 / `RunVelocity` 9; the shared synthetic set is
//! unchanged.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use d2_client::app::server_thread::ThreadLink;
use d2_client::app::single_player::{self, GameData, Link, DEFAULT_SEED};
use d2_client::app::skill_rest::SkillStore;
use d2_client::bridge::link::{SendQueue, ServerLink};
use d2_client::bridge::LOCAL_CLIENT;
use d2_data::tables::{Charstats, Monstats, Monstats2, Record, Skills};
use d2_proto::client::Walk;
use d2_server::seams::{Clock, Pos};

struct StepClock(Arc<AtomicU32>);

impl Clock for StepClock {
    fn now_ms(&mut self) -> u32 {
        self.0.load(Ordering::SeqCst)
    }
}

/// A `charstats` row with the walk and run velocities (the synthetic
/// game has no `charstats`, so its players stand still: velocity 0).
fn charstats_row() -> Charstats {
    let mut c = Charstats::decode(&[0u8; Charstats::SIZE]);
    c.walkvelocity = 6;
    c.runvelocity = 9;
    c
}

struct Game {
    link: ThreadLink<Link<StepClock>>,
    ms: Arc<AtomicU32>,
}

impl Game {
    /// The game after the session sequence (0x67, 0x6B) and two more
    /// ticks: the player is in its room.
    fn joined() -> Self {
        let ms = Arc::new(AtomicU32::new(1000));
        let (link, _) =
            single_player::start(GameData::Synthetic, DEFAULT_SEED, StepClock(ms.clone())).unwrap();
        let mut g = Self { link, ms };
        // The test-local skill rows (as `app_server_skills.rs`): eight
        // zero `skills` records, so the join gives the player skill 0
        // (Attack) in both hands.
        g.link
            .with(|l| {
                let h = l.host_mut().game.events.action.hooks();
                let mut t = (*h.tables).clone();
                t.skills.skills = vec![Skills::decode(&[0u8; Skills::SIZE]); 8];
                // Attack: `anim` A1 (mode 7), `range` h2h (1).
                t.skills.skills[0].anim = 7;
                t.skills.skills[0].range = 1;
                t.skills.level_cap = d2_sim::skills::LEVEL_CAP_114D;
                h.tables = Arc::new(t);
                let mut store = SkillStore::from_tables(&h.tables);
                store.class_skills = vec![[0xFFFF; 10]; 7];
                h.x.skills = store;
            })
            .unwrap();
        let req = single_player::create_request();
        g.link.send(SendQueue::System, &req.encode()).unwrap();
        g.ticks(1);
        g.link.send(SendQueue::System, &[0x6B]).unwrap();
        g.ticks(3);
        g.link
            .with(|l| {
                let h = &mut l.host_mut().game.events.action.sys.hooks;
                let mut t = (*h.tables).clone();
                t.combat.charstats = (0..7).map(|_| charstats_row()).collect();
                t.combat.monstats = vec![Monstats::decode(&[0u8; Monstats::SIZE])];
                t.combat.monstats2 = vec![Monstats2::decode(&[0u8; Monstats2::SIZE])];
                h.tables = Arc::new(t);
            })
            .unwrap();
        g
    }

    /// Pumps `n` ticked frames; returns every S→C chunk received.
    fn ticks(&mut self, n: usize) -> Vec<Vec<u8>> {
        let mut got = Vec::new();
        for _ in 0..n {
            self.link.pump().unwrap();
            self.ms.fetch_add(40, Ordering::SeqCst);
            self.link.pump().unwrap();
            got.extend(self.link.receive());
        }
        got
    }

    /// The server-side position of the local player (`SimGame::player_pos`).
    fn player_pos(&mut self) -> Option<Pos> {
        self.link
            .with(|l| l.host().game.player_pos(LOCAL_CLIENT))
            .unwrap()
    }
}

// Covers: specs/sim/intents-events.md §2.4 r3
#[test]
fn the_server_side_player_walks_on_a_client_walk() {
    let mut g = Game::joined();
    let start = g
        .player_pos()
        .expect("the player's facts come from the sim");
    assert_ne!(start, Pos { x: 0, y: 0 }, "placed by game entry");
    let to = Walk {
        x: (start.x + 4) as u16,
        y: start.y as u16,
    };
    g.link.send(SendQueue::Game, &to.encode()).unwrap();
    g.ticks(25);
    let end = g.player_pos().unwrap();
    assert!(
        end.x > start.x,
        "the server moved the player: {start:?} → {end:?}"
    );
}

/// A monster of class 0 allocated by the server next to the player, in
/// the player's room; its GUID.
fn monster_next_to_player(g: &mut Game) -> u32 {
    use d2_sim::units::lifecycle::AllocRequest;
    use d2_sim::units::UnitType;
    g.link
        .with(|l| {
            let s = &mut l.host_mut().game;
            let (p, _) = single_player::local_player(s).expect("joined");
            let room = s.game.lists.unit(p).and_then(|e| e.room());
            let pos = s.player_pos(LOCAL_CLIENT).unwrap();
            let monsters = &mut s.events.action.sys.data.monsters;
            if monsters.is_empty() {
                monsters.push(d2_sim::units::hooks::MonsterInfo {
                    enabled: true,
                    ..Default::default()
                });
            }
            let req = AllocRequest {
                ty: UnitType::Monster,
                class: 0,
                room,
                add: true,
                fixed_guid: None,
                mode: 1,
                allied: false,
            };
            let m = s
                .events
                .action
                .with(&mut s.game, |gm, v| v.allocate(gm, &req, pos.x + 2, pos.y))
                .unwrap_or_else(|| {
                    let h = s.events.action.hooks();
                    panic!("monster allocated: {:?} {:?}", h.x.log, h.errors)
                });
            s.game.lists.unit(m).unwrap().guid
        })
        .unwrap()
}

// Covers: specs/sim/intents-events.md §2.4 r4
#[test]
fn a_left_skill_on_a_monster_next_to_the_player_starts_the_attack() {
    let mut g = Game::joined();
    let guid = monster_next_to_player(&mut g);
    let mut msg = vec![0x06, 1, 0, 0, 0];
    msg.extend(guid.to_le_bytes());
    g.link.send(SendQueue::Game, &msg).unwrap();
    g.link.pump().unwrap();
    let handled = g
        .link
        .with(|l| format!("{:?}", l.last_frame().messages))
        .unwrap();
    // Before: no facts for the monster → `Refused`; no skill slot → the
    // stub; no left skill → `Malformed` (code 3).
    assert!(handled.contains("Dispatched(Done)"), "{handled}");
    g.ticks(2);
    let (log, errors) = g
        .link
        .with(|l| {
            let h = l.host_mut().game.events.action.hooks();
            (h.x.skills.log.clone(), format!("{:?}", h.errors))
        })
        .unwrap();
    // In melee reach: the skill's mode starts at once (`use.md` §3), no
    // run to the target. The synthetic game has no animdata, so the
    // attack animation itself fails here (`Anim(NoRecord)`); with the
    // user's files it runs (local check, docs/handoff/stitch-server-core.md).
    assert!(log.iter().all(|l| !l.starts_with("run to")), "{log:?}");
    assert!(errors.contains("NoRecord"), "{errors}");
}
