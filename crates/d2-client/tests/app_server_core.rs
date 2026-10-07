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
use d2_client::bridge::link::{SendQueue, ServerLink};
use d2_client::bridge::LOCAL_CLIENT;
use d2_data::tables::{Charstats, Record};
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
