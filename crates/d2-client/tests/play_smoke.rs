// Spec: specs/client/bridge.md (§2, §4, §6, §8), specs/sim/intents-events.md (§2.2, §8), specs/flows/save-exit.md (§1, §2, §4, §5); preview fills: docs/PLAN.md decisions D1–D3, docs/handoff/q-play-smoke.md
//! The end-to-end play smoke run (task `q-play-smoke`, moved onto the
//! user's install by q-fixture-migrate): one scripted run through the
//! real play path, headless (the bridge, the in-process server and the
//! sim, the play app's client as `play::add_live_client` wires it for
//! `d2-client play`, no window), on the user's 1.14d install
//! (`$D2_GAME_DIR`). Every table, level, tile and UI file is the
//! install's; nothing is invented. The tests are `#[ignore]`: the
//! real-data gate runs them (`tools/realdata-gate.sh`).
//!
//! After every step the run asserts that the server refused no intent
//! (every drained C→S message dispatched with result 0, `seams.rs`
//! `ResultCode::Done`), that the client model dropped, rejected,
//! discarded and left unhandled no S→C message (`bridge.md` §6), and no
//! system panicked (Bevy's error handler panics on a bridge error).

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use d2_client::app::play::{add_live_client, LiveClient};
use d2_client::app::server_thread::ThreadLink;
use d2_client::app::single_player::{self, GameData};
use d2_client::bridge::link::{LinkError, Pumped, SendQueue, Sent, ServerLink};
use d2_client::bridge::local::{LocalLink, PendingSession};
use d2_client::bridge::world::UnitKey;
use d2_client::bridge::BridgeResource;
use d2_client::controls::Action;
use d2_client::ui::layout::OptionKind;
use d2_client::ui::panel::ActionId;
use d2_client::ui::{Point, PointerButton, UiEvent};
use d2_client::world_view::WorldViewUi;
use d2_server::adapters::ProtoSizes;
use d2_server::dispatch::Outcome;
use d2_server::host::Handled;
use d2_server::host::Host;
use d2_server::seams::{Clock, ResultCode};

mod app_support;
use app_support::{Server, SharedLink};

struct StepClock(Arc<AtomicU32>);

impl Clock for StepClock {
    fn now_ms(&mut self) -> u32 {
        self.0.load(Ordering::SeqCst)
    }
}

/// What crossed the link: the C→S messages sent and every drained
/// message the server did not accept.
#[derive(Default)]
struct Wire {
    sent: Vec<Vec<u8>>,
    refused: Vec<(u8, Handled)>,
    /// The ids of every S→C message received.
    received: Vec<u8>,
    /// Every drained C→S message's id and fate.
    drained: Vec<(u8, Handled)>,
}

/// The app's link: the shared server thread, recording both directions'
/// fates.
struct Probe {
    server: Server<StepClock>,
    wire: Arc<Mutex<Wire>>,
}

impl ServerLink for Probe {
    fn protocol_version(&self) -> u32 {
        SharedLink(self.server.clone()).protocol_version()
    }
    fn send(&mut self, queue: SendQueue, msg: &[u8]) -> Result<Sent, LinkError> {
        self.wire.lock().unwrap().sent.push(msg.to_vec());
        SharedLink(self.server.clone()).send(queue, msg)
    }
    fn pump(&mut self) -> Result<Pumped, LinkError> {
        let p = SharedLink(self.server.clone()).pump()?;
        let drained = app_support::with(&self.server, |l| l.last_frame().messages.clone());
        let mut w = self.wire.lock().unwrap();
        for m in drained {
            w.drained.push((m.id, m.handled));
            let ok = matches!(
                m.handled,
                Handled::System | Handled::Game(Outcome::Dispatched(ResultCode::Done))
            );
            if !ok {
                w.refused.push((m.id, m.handled));
            }
        }
        Ok(p)
    }
    fn receive(&mut self) -> Vec<Vec<u8>> {
        let got = SharedLink(self.server.clone()).receive();
        let mut w = self.wire.lock().unwrap();
        for c in &got {
            // Chunk ids only (the bridge splits them); enough for a trace.
            if let Some(&id) = c.first() {
                w.received.push(id);
            }
        }
        got
    }
}

/// The run: the app, its server thread, the host clock and the wire.
struct Run {
    app: App,
    server: Server<StepClock>,
    ms: Arc<AtomicU32>,
    wire: Arc<Mutex<Wire>>,
    frames: usize,
    character: single_player::Character,
    /// Divergences met on the way (a run leg drawn away from where the
    /// server's player stopped, a step the server did not carry out):
    /// asserted empty at the end of a run, so one divergence does not
    /// hide the steps after it. Each names its finding row.
    findings: Vec<String>,
}

/// A character's state for the save round trip: (stat, value), (skill,
/// base level), (item code, mode, body location).
/// The waypoint records and quest flag records of the three
/// difficulties.
type Snapshot = (
    Vec<(u16, i32)>,
    Vec<(u16, i32)>,
    Vec<([u8; 4], u8, u8)>,
    Option<[[u8; 16]; 3]>,
    Option<Vec<[u8; 96]>>,
);

impl Run {
    /// The play app on the user's install, as `d2-client play --new
    /// sorceress Smoke` wires it; returns once the join ran.
    fn start() -> Self {
        let character = single_player::new_character("sorceress", "Smoke").unwrap();
        Self::start_as(character)
    }

    /// [`Self::start`] with `character` (a new one, or a save).
    fn start_as(character: single_player::Character) -> Self {
        let data = app_support::game_data();
        let GameData::Live(live) = data.clone();
        let ms = Arc::new(AtomicU32::new(1000));
        let clock = StepClock(ms.clone());
        let built = character.clone();
        let game_data = data.clone();
        // The seed `d2-client play` builds with when no `--seed` is given:
        // a save's map seed, else the default (`game_seed`).
        let seed = single_player::game_seed(&character, None);
        let (tx, rx) = std::sync::mpsc::channel();
        let link = ThreadLink::spawn(move || {
            let g = single_player::build_with(&game_data, seed, built)?;
            let _ = tx.send(g.sim.world.rest.prices.clone());
            Ok::<_, single_player::BuildError>(LocalLink::new(Host::new(
                g.sim,
                ProtoSizes,
                PendingSession::default(),
                clock,
            )))
        })
        .unwrap();
        let prices = rx.recv().expect("the game built");
        let server = Arc::new(Mutex::new(link));
        let wire = Arc::new(Mutex::new(Wire::default()));
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Image>()
            .init_resource::<ButtonInput<MouseButton>>();
        let probe = Probe {
            server: server.clone(),
            wire: wire.clone(),
        };
        let speeds = single_player::walk_speeds(&data, &character).unwrap();
        assert!(speeds.is_some(), "walk speeds from charstats");
        add_live_client(
            &mut app,
            Box::new(probe),
            LiveClient {
                data: &live,
                request: &character,
                start_flags: None,
                prices,
                speeds,
                hardcore: false,
                automap_files: None,
                gpu: false,
            },
        )
        .unwrap();
        let mut run = Run {
            app,
            server,
            ms,
            wire,
            frames: 0,
            character,
            findings: Vec::new(),
        };
        while app_support::local_player(&run.server).is_none() {
            run.step(1);
        }
        run.step(30);
        run.check("join");
        run
    }

    fn step(&mut self, n: usize) {
        for _ in 0..n {
            self.app.update();
            self.ms.fetch_add(40, Ordering::SeqCst);
            self.frames += 1;
            assert!(self.frames < 100_000, "the run finishes");
        }
    }

    /// Steps until `done` or `limit` frames.
    fn until(&mut self, what: &str, limit: usize, mut done: impl FnMut(&mut Run) -> bool) {
        for _ in 0..limit {
            if done(self) {
                return;
            }
            self.step(1);
        }
        assert!(done(self), "{what}: not reached in {limit} frames");
    }

    fn bridge(&mut self) -> &mut d2_client::bridge::Bridge<d2_client::bridge::mirror::DynLink> {
        &mut self
            .app
            .world_mut()
            .resource_mut::<BridgeResource>()
            .into_inner()
            .0
    }

    fn queue(&mut self, e: UiEvent) {
        self.app
            .world_mut()
            .non_send_mut::<WorldViewUi>()
            .queue
            .0
            .push(e);
    }

    fn click(&mut self, at: Point) {
        for press in [true, false] {
            let button = PointerButton::Left;
            self.queue(if press {
                UiEvent::Press { button, at }
            } else {
                UiEvent::Release { button, at }
            });
        }
        self.step(2);
    }

    /// The town waypoint to Cold Plains: interact, the menu opens, C→S
    /// 0x49, the server player is there.
    fn waypoint_to_cold_plains(&mut self) {
        // A waypoint: an `objects` row with operate function 23
        // (`waypoints.md` §5.1), the nearest to the server player.
        let rows = app_support::live().waypoints.objects.clone();
        let (px, py) = self.pos();
        let wp = {
            let w = self.app.world().resource::<BridgeResource>().0.world();
            w.units
                .iter()
                .filter(|(k, u)| {
                    k.unit_type == 2
                        && u.position.is_some()
                        && rows
                            .get(u.class as usize)
                            .is_some_and(|r| r.operatefn == 23)
                })
                .min_by_key(|(_, u)| {
                    let (x, y) = u.position.unwrap();
                    (i32::from(x) - px).abs().max((i32::from(y) - py).abs())
                })
                .map(|(k, _)| *k)
                .expect("the waypoint in the model")
        };
        self.run_to_unit(wp);
        self.bridge().interact(wp).unwrap();
        self.until("the waypoint menu", 400, |r| r.ui_open(0x14));
        self.check("open the waypoint");
        let mut take = vec![0x49];
        take.extend_from_slice(&wp.guid.to_le_bytes());
        take.extend_from_slice(&single_player::COLD_PLAINS.to_le_bytes());
        self.bridge().send_bytes(&take).unwrap();
        self.until("Cold Plains", 400, |r| {
            r.server_level() == Some(single_player::COLD_PLAINS)
        });
        self.step(30);
        self.check("waypoint to Cold Plains");
    }

    fn ui_open(&self, ui: u8) -> bool {
        self.app
            .world()
            .non_send::<WorldViewUi>()
            .original
            .as_ref()
            .unwrap()
            .is_open(ui)
    }

    /// The step's checks (module docs).
    fn check(&mut self, step: &str) {
        let w = self.wire.lock().unwrap();
        assert!(
            w.refused.is_empty(),
            "{step}: the server refused {:02X?}",
            w.refused
        );
        // No position resync asked: the server never walks the player
        // to a client guess (the rubber band, REC-277).
        assert!(
            !w.sent.iter().any(|m| m.first() == Some(&0x5F)),
            "{step}: the client sent C→S 0x5F"
        );
        drop(w);
        let b = &self.app.world().resource::<BridgeResource>().0;
        let log = b.log();
        assert!(
            log.unowned.is_empty(),
            "{step}: unhandled {:02X?}",
            log.unowned
        );
        assert!(
            log.dropped.is_empty(),
            "{step}: dropped {:02X?}",
            log.dropped
        );
        assert!(
            log.rejected.is_empty(),
            "{step}: rejected {:?}",
            log.rejected
        );
        assert!(
            log.discarded.is_empty(),
            "{step}: discarded {:?}",
            log.discarded
        );
    }

    fn find(&self, ty: u8, class: u32) -> Option<UnitKey> {
        self.app
            .world()
            .resource::<BridgeResource>()
            .0
            .world()
            .units
            .iter()
            .find(|(k, u)| k.unit_type == ty && u.class == class && u.position.is_some())
            .map(|(k, _)| *k)
    }

    /// The server's unit of type `ty` and class `class` and its sub-tile.
    fn server_unit(&self, ty: u8, class: u32) -> Option<(i32, i32)> {
        let st = match ty {
            1 => d2_sim::units::UnitType::Monster,
            _ => d2_sim::units::UnitType::Object,
        };
        app_support::with(&self.server, move |l| {
            let g = &mut l.host_mut().game;
            let u = g.game.lists.units_of_type(st).into_iter().find(|&u| {
                g.events
                    .action
                    .sys
                    .units
                    .get(u)
                    .is_some_and(|r| r.class == class)
            })?;
            Some(g.events.action.hooks().path_position(u))
        })
    }

    /// Run legs toward `goal` until `done`, within 10 sub-tiles of it, or
    /// stuck.
    fn run_to(&mut self, goal: (i32, i32), mut done: impl FnMut(&mut Run) -> bool) -> bool {
        use test_fixtures::host::{cheb, LEG};
        for _ in 0..30 {
            if done(self) {
                return true;
            }
            let p = self.pos();
            if cheb(p, goal) <= 10 {
                return done(self);
            }
            self.leg((
                p.0 + (goal.0 - p.0).clamp(-LEG, LEG),
                p.1 + (goal.1 - p.1).clamp(-LEG, LEG),
            ));
            if cheb(self.pos(), p) <= 2 && !self.sidestep() {
                break;
            }
        }
        done(self)
    }

    /// The town's DRLG room centres (sub-tiles), nearest to the player
    /// first.
    fn town_rooms(&self) -> Vec<(i32, i32)> {
        let p = self.pos();
        let mut v: Vec<(i32, i32)> = app_support::with(&self.server, |l| {
            let g = &mut l.host_mut().game;
            g.events
                .action
                .hooks()
                .drlg
                .with_act(0, &mut g.game.lists, |d, _| {
                    let l = d.find_level(single_player::ACT1_TOWN)?;
                    Some(
                        d.level_rooms(l)
                            .into_iter()
                            .map(|r| {
                                let t = d.room(r).rect;
                                ((t.x * 2 + t.w) * 5 / 2, (t.y * 2 + t.h) * 5 / 2)
                            })
                            .collect::<Vec<_>>(),
                    )
                })
                .flatten()
                .unwrap_or_default()
        });
        v.sort_by_key(|&c| test_fixtures::host::cheb(c, p));
        v
    }

    /// Walks the town until the client model holds the unit of type `ty`
    /// and class `class` within 10 sub-tiles of the player: toward it
    /// once the server has it, else through the town's rooms (their
    /// presets are placed when the player brings them into play).
    fn approach(&mut self, ty: u8, class: u32) -> UnitKey {
        let near = move |r: &mut Run| {
            let p = r.pos();
            r.find(ty, class).is_some()
                && r.server_unit(ty, class)
                    .is_some_and(|at| test_fixtures::host::cheb(p, at) <= 10)
        };
        let mut rooms = self.town_rooms().into_iter();
        for _ in 0..40 {
            if near(self) {
                return self.find(ty, class).unwrap();
            }
            match self.server_unit(ty, class) {
                Some(at) => {
                    self.run_to(at, near);
                }
                None => {
                    let Some(c) = rooms.next() else { break };
                    self.run_to(c, move |r| r.server_unit(ty, class).is_some());
                }
            }
        }
        self.dump("approach");
        panic!("unit {ty}/{class} not reached");
    }

    /// Runs to the unit `key` as the client does before an interact
    /// (C→S 0x04, run to a unit), until the player stops.
    fn run_to_unit(&mut self, key: UnitKey) {
        let mut m = vec![0x04];
        m.extend_from_slice(&u32::from(key.unit_type).to_le_bytes());
        m.extend_from_slice(&key.guid.to_le_bytes());
        self.bridge().send_bytes(&m).unwrap();
        self.step(2);
        for _ in 0..300 {
            if !test_fixtures::host::MOVING.contains(&self.mode()) {
                break;
            }
            self.step(1);
        }
        self.step(4);
        self.check("run to the unit");
    }

    /// No divergence was met ([`Run::findings`]).
    fn no_findings(&self) {
        assert!(
            self.findings.is_empty(),
            "{} divergences:\n{}",
            self.findings.len(),
            self.findings.join("\n")
        );
    }

    /// What the server and the client model hold (diagnostics).
    fn dump(&self, at: &str) {
        let server = app_support::with(&self.server, |l| {
            let g = &mut l.host_mut().game;
            let mut v = Vec::new();
            for ty in [
                d2_sim::units::UnitType::Monster,
                d2_sim::units::UnitType::Object,
            ] {
                for u in g.game.lists.units_of_type(ty) {
                    let class = g.events.action.sys.units.get(u).map(|r| r.class);
                    let p = g.events.action.hooks().path_position(u);
                    v.push((ty as u8, class, p));
                }
            }
            v
        });
        let w = self.app.world().resource::<BridgeResource>().0.world();
        let client: Vec<_> = w
            .units
            .iter()
            .map(|(k, u)| (k.unit_type, u.class, u.position))
            .collect();
        eprintln!(
            "{at}: player {:?}\n server {server:?}\n client {client:?}",
            self.pos()
        );
    }

    fn server_level(&self) -> Option<u32> {
        app_support::with(&self.server, |l| {
            let (p, _) = single_player::local_player(&l.host().game)?;
            let g = &mut l.host_mut().game;
            let room = g.game.lists.unit(p)?.room()?;
            g.events.action.hooks().drlg.level_id(&g.game, room)
        })
    }
}

/// The NPC menu row of `kind`, clicked (`npc_menu_ui.rs`: the box is
/// centred, a quarter down the 800 × 600 frame, rows of 20).
fn menu_row_point(rows: &[Option<OptionKind>], kind: Option<OptionKind>) -> Point {
    let k = rows.iter().position(|r| *r == kind).expect("the row") as i32;
    Point::new((800 - 200) / 2 + 20, 600 / 4 + 20 * (k + 1) + 10)
}

#[test]
#[ignore = "real data: needs D2_GAME_DIR (tools/realdata-gate.sh)"]
fn the_scripted_play_run() {
    let mut run = Run::start();

    // 1. The new character: a player with a skill list (no injected
    // S→C 0x94 / 0x23).
    {
        let w = run.app.world().resource::<BridgeResource>().0.world();
        let p = w.local().expect("the local player");
        assert!(p.position.is_some(), "placed");
    }
    run.check("new character");

    // 2. Talk to Akara: run toward where the town's preset placed her
    // until the client holds her, then the interact sender (the preview's
    // arrival).
    let akara = run.approach(1, u32::from(d2_sim::world::npc::class::AKARA));
    run.run_to_unit(akara);
    run.bridge().interact(akara).unwrap();
    run.until("the NPC menu", 400, |r| {
        r.app
            .world()
            .non_send::<WorldViewUi>()
            .original
            .as_ref()
            .unwrap()
            .npc_menu()
            .is_some()
    });
    run.check("talk to Akara");
    let rows: Vec<Option<OptionKind>> = run
        .app
        .world()
        .non_send::<WorldViewUi>()
        .original
        .as_ref()
        .unwrap()
        .npc_menu()
        .unwrap()
        .rows
        .iter()
        .map(|r| r.kind)
        .collect();
    eprintln!("Akara's menu: {rows:?}");
    run.click(menu_row_point(&rows, Some(OptionKind::Trade)));
    run.step(10);
    run.check("trade with Akara");
    {
        let w = run.app.world().resource::<BridgeResource>().0.world();
        let skills = w
            .local()
            .and_then(|p| p.skills.as_ref())
            .map(|s| format!("{s:?}"));
        eprintln!("skills: {skills:?}");
        eprintln!(
            "store items: {}, local items: {}",
            d2_client::bridge::items::store_items(w).len(),
            d2_client::bridge::items::local_items(w).len()
        );
    }
    // The Trade choice opens the shop even with an empty store (REC-277),
    // and its close ends the interaction with C→S 0x30.
    assert!(run.ui_open(0x0C), "the shop opened on the Trade choice");
    run.app
        .world_mut()
        .non_send_mut::<WorldViewUi>()
        .original
        .as_mut()
        .unwrap()
        .set_ui(0x0C, 1, false)
        .unwrap();
    run.step(10);
    // C→S 0x30: the server reads the NPC GUID at +5 only
    // (`client-messages.tsv`).
    let ended = run
        .wire
        .lock()
        .unwrap()
        .sent
        .iter()
        .any(|m| m.len() == 9 && m[0] == 0x30 && m[5..9] == akara.guid.to_le_bytes());
    assert!(ended, "closing the shop sent C→S 0x30 for Akara");
    run.check("close the shop");

    // 3. The waypoint to Cold Plains.
    run.waypoint_to_cold_plains();
    let units: Vec<(u8, u32)> = run
        .app
        .world()
        .resource::<BridgeResource>()
        .0
        .world()
        .units
        .iter()
        .map(|(k, u)| (k.unit_type, u.class))
        .collect();
    eprintln!("units in the field: {units:?}");
    run.no_findings();
}

/// The play app's build (`GameData::select` on the install, every act
/// created, `LevelSource::live`) runs on the user's files.
#[test]
#[ignore = "real data: needs D2_GAME_DIR (tools/realdata-gate.sh)"]
fn the_play_game_builds_on_the_install() {
    let data = app_support::game_data();
    let g = single_player::build(&data, single_player::DEFAULT_SEED);
    assert!(g.is_ok(), "{:?}", g.err());
}

impl Run {
    /// The server player's path position (sub-tiles).
    fn pos(&self) -> (i32, i32) {
        app_support::with(&self.server, |l| {
            let g = &mut l.host_mut().game;
            let (p, _) = single_player::local_player(g).unwrap();
            g.events.action.hooks().path_position(p)
        })
    }

    fn mode(&self) -> u32 {
        app_support::with(&self.server, |l| {
            let g = &l.host().game;
            let (p, _) = single_player::local_player(g).unwrap();
            g.events.action.sys.units.get(p).map_or(0, |u| u.mode)
        })
    }

    /// The tile rect of level `id` in the act of the player.
    fn level_rect(&self, id: u32) -> d2_sim::drlg::TileRect {
        app_support::with(&self.server, move |l| {
            let h = l.host_mut().game.events.action.hooks();
            h.drlg
                .dungeon
                .acts
                .iter()
                .flatten()
                .find_map(|d| d.find_level(id).map(|lv| d.level(lv).rect))
                .expect("level allocated")
        })
    }

    /// A run leg (C→S 0x03 through the bridge), then frames until the
    /// player stops.
    fn leg(&mut self, (x, y): (i32, i32)) {
        let mut m = vec![0x03];
        m.extend_from_slice(&(x as u16).to_le_bytes());
        m.extend_from_slice(&(y as u16).to_le_bytes());
        self.bridge().send_bytes(&m).unwrap();
        self.step(2);
        for _ in 0..400 {
            if !test_fixtures::host::MOVING.contains(&self.mode()) {
                break;
            }
            self.step(1);
        }
        // The walk prediction steps its last ticks (a frame behind).
        self.step(10);
        let (drawn, p) = (self.drawn(), self.pos());
        if test_fixtures::host::cheb(drawn, p) > 2 {
            eprintln!("run leg to ({x}, {y}): drawn {drawn:?}, server {p:?}");
            self.findings.push(format!(
                "q-fix-real-client-path: run leg to ({x}, {y}) drawn at {drawn:?}, server at {p:?}"
            ));
        }
        self.check("run leg");
    }

    /// The walk prediction's sub-tile: where the client draws its player.
    fn drawn(&self) -> (i32, i32) {
        self.app
            .world()
            .resource::<d2_client::world_view::walk::PreviewWalk>()
            .predict
            .cell()
            .map(|(x, y)| (i32::from(x), i32::from(y)))
            .expect("a prediction")
    }

    /// The shared collision route (`test_fixtures::host::route`) from the
    /// server player toward level `to`, inside levels `from` and `to`.
    fn route(&self, from: u32, to: u32) -> Vec<(i32, i32)> {
        let (fr, tr) = (self.level_rect(from), self.level_rect(to));
        let start = self.pos();
        app_support::with(&self.server, move |l| {
            let h = l.host_mut().game.events.action.hooks();
            let d = h.drlg.dungeon.acts[0].as_ref().expect("Act I");
            test_fixtures::host::route(d, start, fr, tr, 12)
        })
    }

    /// Run legs into level `to` (from `from`) along [`Self::route`] until
    /// the server player is there (the route is searched again as the
    /// rooms ahead become active).
    fn walk_into(&mut self, from: u32, to: u32) {
        for _ in 0..12 {
            if self.server_level() == Some(to) {
                return;
            }
            let route = self.route(from, to);
            for g in route {
                self.leg(g);
                if self.server_level() == Some(to) {
                    break;
                }
            }
        }
        assert_eq!(self.server_level(), Some(to), "walked into level {to}");
    }
}

/// The scripted run on the five-act install, loaded as `d2-client play`
/// loads the user's files: join, then out of the Rogue Encampment into
/// the Blood Moor on foot (run legs, C→S 0x03). It failed twice on the way:
/// the position checks of S→C 0x96 had no visibility predicate
/// (`model.md` §13 r6: now the world view's), and a check correction sent
/// C→S 0x5F with a client position the server had not walked (the last
/// placement, or the straight-line prediction past an obstacle), so the
/// server walked the player there: the rubber band. While the preview
/// predicts, the check now follows the server (REC-277); `Run::check`
/// asserts no 0x5F is ever sent.
#[test]
#[ignore = "real data: needs D2_GAME_DIR (tools/realdata-gate.sh)"]
fn the_live_run() {
    let data = app_support::game_data();
    let dir = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("smoke-{}-live", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut run = Run::start();
    run.walk_into(1, 2);
    run.step(20);
    run.check("Blood Moor");
    assert_eq!(run.server_level(), Some(2), "out of town on foot");
    // Spawned monsters, killed with the left skill (Attack), until one
    // drops something.
    let mut ground = Vec::new();
    for n in 0..8 {
        let near = run.monsters_near();
        let target = *near.first().expect("a monster spawned in the Blood Moor");
        let killed = run.kill(target);
        assert!(
            killed,
            "monster {n} died: life {:?}",
            run.monster_life(target.guid)
        );
        run.step(30);
        run.check("kill");
        ground = run.ground_items();
        let xp = run.player_stat(13);
        eprintln!(
            "kill {n}: experience {xp}, ground {:?}",
            ground
                .iter()
                .map(|i| (i.key.guid, i.code))
                .collect::<Vec<_>>()
        );

        if !ground.is_empty() {
            break;
        }
    }
    assert!(!ground.is_empty(), "a kill dropped an item");

    // Pick up every drop (C→S 0x16, auto placement: the gold to the
    // player's gold, the items to the belt or the inventory), running to
    // it first as the client does (C→S 0x04, run to the unit).
    let gold0 = run.player_stat(14);
    for it in &ground {
        let p = run.pos();
        if (p.0 - i32::from(it.x))
            .abs()
            .max((p.1 - i32::from(it.y)).abs())
            > 2
        {
            let mut m = vec![0x04];
            m.extend_from_slice(&4u32.to_le_bytes());
            m.extend_from_slice(&it.key.guid.to_le_bytes());
            run.bridge().send_bytes(&m).unwrap();
            run.step(2);
            for _ in 0..300 {
                if !test_fixtures::host::MOVING.contains(&run.mode()) {
                    break;
                }
                run.step(1);
            }
            run.check("run to the item");
        }
        run.bridge()
            .send(&d2_client::bridge::items::pick(it.key.guid, false))
            .unwrap();
        run.step(6);
        run.check("pick up");
    }
    let left: Vec<_> = run
        .ground_items()
        .iter()
        .map(|i| (i.key.guid, i.code))
        .collect();
    assert!(left.is_empty(), "everything picked up: {left:?}");
    assert!(run.player_stat(14) > gold0, "the gold reached the player");
    let mine = run.local_items();
    for it in ground.iter().filter(|i| i.code != Some(*b"gld ")) {
        assert!(
            mine.iter().any(|m| m.key == it.key),
            "{:?} is the player's: {mine:?}",
            it.code
        );
    }

    // Level up: more kills until level 2 (500 experience, the install's
    // `experience` row 2), searching deeper into the Blood Moor when none
    // is near.
    let rect = run.level_rect(2);
    let centre = (
        (rect.x + rect.w / 2) * test_fixtures::host::SUB,
        (rect.y + rect.h / 2) * test_fixtures::host::SUB,
    );
    let mut unreachable = Vec::new();
    for i in 0..120 {
        if run.player_stat(12) >= 2 {
            break;
        }
        let near = run.monsters_near();
        eprintln!(
            "level-up {i}: frame {}, experience {}, {} monsters near, unreachable {}",
            run.frames,
            run.player_stat(13),
            near.len(),
            unreachable.len()
        );
        match near.into_iter().find(|m| !unreachable.contains(&m.guid)) {
            Some(m) => {
                if !run.kill(m) {
                    unreachable.push(m.guid);
                    continue;
                }
                run.step(20);
                run.check("kill");
            }
            None => {
                // Toward the level's centre first, then the other
                // directions, until a leg moves (a blocked path stays).
                let p = run.pos();
                let toward = (
                    (centre.0 - p.0).signum() * 15,
                    (centre.1 - p.1).signum() * 15,
                );
                let dirs = [
                    toward,
                    (15, 0),
                    (0, 15),
                    (-15, 0),
                    (0, -15),
                    (15, 15),
                    (-15, 15),
                    (15, -15),
                    (-15, -15),
                ];
                for d in dirs {
                    run.leg((p.0 + d.0, p.1 + d.1));
                    if test_fixtures::host::cheb(run.pos(), p) > 3 {
                        break;
                    }
                }
            }
        }
    }
    assert_eq!(run.player_stat(12), 2, "level 2");
    // The level's stat points: the install's `charstats` StatPerLevel
    // of the sorceress (5); one skill point a level (`vitals.md`).
    let per_level = {
        let GameData::Live(d) = &data;
        let rows: Vec<d2_data::tables::Charstats> = d.tables.rows().unwrap();
        i32::from(rows[1].statperlevel)
    };
    let (points, skills) = (run.player_stat(4), run.player_stat(5));
    assert_eq!(
        (points, skills),
        (per_level, 1),
        "the level's stat and skill points"
    );
    // Spend: strength + 1 (C→S 0x3A stat 0, count − 1 = 0), Fire Bolt
    // (skill 36 of the install's `skills`, the sorceress' level-1 class
    // skill) + 1 (0x3B).
    let str0 = run.player_stat(0);
    run.bridge().send_bytes(&[0x3A, 0, 0]).unwrap();
    run.bridge().send_bytes(&[0x3B, 36, 0]).unwrap();
    run.step(6);
    run.check("spend points");
    assert_eq!(run.player_stat(0), str0 + 1, "strength");
    assert_eq!(run.player_stat(4), points - 1, "a stat point spent");
    assert_eq!(run.player_stat(5), skills - 1, "a skill point spent");
    let firebolt = {
        let w = run.app.world().resource::<BridgeResource>().0.world();
        w.local()
            .and_then(|p| p.skills.as_ref())
            .map(|l| format!("{l:?}"))
    };
    assert!(
        firebolt.is_some_and(|l| l.contains("skill: 36,")),
        "the client learned Firebolt (S→C 0x21)"
    );

    // Town Portal: the start scroll used (C→S 0x20 at the player's
    // position), the field portal reaches the client (S→C 0x51), a click
    // on it (0x13) takes the player to the Rogue Encampment.
    let scroll = run
        .local_items()
        .into_iter()
        .find(|i| i.code == Some(*b"tsc "))
        .expect("the start scroll")
        .key
        .guid;
    let p = run.pos();
    run.bridge()
        .send(&d2_client::bridge::items::use_grid(
            scroll, p.0 as u32, p.1 as u32,
        ))
        .unwrap();
    run.step(10);
    run.check("read the scroll");
    let portal = {
        let w = run.app.world().resource::<BridgeResource>().0.world();
        w.units
            .iter()
            .find(|(k, u)| k.unit_type == 2 && u.class == 59 && u.position.is_some())
            .map(|(k, _)| *k)
    };
    let portal = portal.expect("the field portal in the client model");
    // The portal's hostile delay (`objects.md` §12 rule 2) has passed.
    run.ms.fetch_add(10_000, Ordering::SeqCst);
    run.bridge().interact(portal).unwrap();
    run.until("the town", 400, |r| r.server_level() == Some(1));
    run.step(20);
    run.check("through the portal");

    // Equip: the start weapon off the right arm to the cursor (C→S 0x1C)
    // and back on (0x1A).
    let weapon = run
        .local_items()
        .into_iter()
        .find(|i| i.mode == 1 && i.body == 4)
        .expect("the start weapon in the right arm")
        .key
        .guid;
    run.bridge()
        .send(&d2_proto::client::RemoveBodyItem { bodyloc: 4 })
        .unwrap();
    run.step(6);
    run.check("unequip");
    let cursor = {
        let w = run.app.world().resource::<BridgeResource>().0.world();
        d2_client::bridge::items::cursor_item(w).map(|i| i.key.guid)
    };
    assert_eq!(cursor, Some(weapon), "the weapon on the cursor");
    run.bridge()
        .send(&d2_client::bridge::items::equip(weapon, 4))
        .unwrap();
    run.step(6);
    run.check("equip");
    let worn = run
        .local_items()
        .into_iter()
        .find(|i| i.mode == 1 && i.body == 4)
        .map(|i| i.key.guid);
    if worn != Some(weapon) {
        // The staff stays on the cursor though 0x1A answers 0 (finding
        // q-fix-real-equip-2h): put it in the inventory (C→S 0x18) so the
        // run goes on.
        run.findings.push(format!(
            "q-fix-real-equip-2h: C→S 0x1A of the start staff (GUID {weapon}) to the right hand answered 0 and left it on the cursor"
        ));
        let mut m = vec![0x18];
        m.extend_from_slice(&weapon.to_le_bytes());
        m.extend_from_slice(&0u32.to_le_bytes());
        m.extend_from_slice(&0u32.to_le_bytes());
        m.extend_from_slice(&0u32.to_le_bytes());
        run.bridge().send_bytes(&m).unwrap();
        run.step(6);
        run.check("to the inventory");
        let cursor = {
            let w = run.app.world().resource::<BridgeResource>().0.world();
            d2_client::bridge::items::cursor_item(w).map(|i| i.key.guid)
        };
        if cursor.is_some() {
            run.findings.push(format!(
                "q-fix-real-equip-2h: C→S 0x18 left the staff (GUID {weapon}) on the cursor too"
            ));
        }
    }

    // Save and exit on the server's path (`flows/save-exit.md`): the
    // server's character storage is the app's file writer (as
    // `play::run`'s `save::share` installs it, with the install's `.d2s`
    // tables); the Esc menu's Save and Exit sends C→S 0x69, the server's
    // leave writes the file before its 0x05, 0x06, and the app ends on
    // them. Then the file is loaded and joined again.
    let GameData::Live(live) = &data;
    let path = dir.join("Smoke.d2s");
    let before = run.snapshot();
    let layouts = |r: &Run| -> Vec<_> { [1u32, 2, 3].map(|l| r.level_rect(l)).to_vec() };
    let layout_before = layouts(&run);
    {
        use d2_client::app::save;
        let store = save::FileStore {
            path: path.clone(),
            base: save::base_save(&run.character),
            tables: Arc::new(live.save.clone()),
        };
        app_support::with(&run.server, move |l| {
            l.host_mut().game.set_storage(Box::new(store))
        });
    }
    assert!(!path.exists(), "nothing written before the leave");
    run.queue(UiEvent::Action(ActionId(Action::GameMenu.index() as u16)));
    run.step(2);
    assert!(run.ui_open(9), "the Esc menu is open");
    // Game menu rows: tops 185 / 235 / 285; Save and Exit is row 1.
    run.click(Point::new(400, 185 + 50 + 20));
    run.until("the app ends on the server's answer", 50, |r| {
        r.app.should_exit().is_some()
    });
    assert!(
        run.wire.lock().unwrap().sent.iter().any(|m| m == &[0x69]),
        "Save and Exit sent C→S 0x69"
    );
    {
        let w = run.app.world().resource::<BridgeResource>().0.world();
        assert!(
            !w.in_game && w.unloaded && w.exit_requested,
            "0x05, 0x06 received"
        );
    }
    let (gone, faults) = app_support::with(&run.server, |l| {
        let g = &l.host().game;
        let gone = g.client_list().is_empty();
        let faults = g.session().map(|s| format!("{:?}", s.faults));
        (gone, faults.unwrap_or_default())
    });
    assert!(gone, "the leave removed the client");
    assert!(!faults.contains("Save"), "the leave saved: {faults}");
    assert!(path.exists(), "the server's leave wrote the save");
    let findings = std::mem::take(&mut run.findings);
    drop(run);
    // The map seed (`d2s.md` §2.1 +0xAB = game +0x7C) is the one the game
    // was built with, and the town byte marks it for restoring (§2.2 r8).
    let character = single_player::load_character(&data, &path, 0).expect("the save loads");
    let single_player::Character::Save(saved, _) = &character else {
        unreachable!("a loaded save")
    };
    assert_eq!(
        saved.header.map_seed,
        single_player::DEFAULT_SEED,
        "the saved map seed"
    );
    assert_eq!(saved.header.towns[0] & 0x80, 0x80, "the Normal town byte");
    assert_eq!(
        single_player::game_seed(&character, None),
        saved.header.map_seed
    );
    let mut run = Run::start_as(character);
    run.check("join the saved character");
    assert_eq!(layouts(&run), layout_before, "the same level layouts");
    let after = run.snapshot();
    assert_eq!(
        (&after.0, &after.1),
        (&before.0, &before.1),
        "the loaded character's stats and skills are the saved ones"
    );
    if after.2 != before.2 {
        // Finding q-fix-real-start-belt: the new sorceress's four `hp1`
        // (charstats item2, count 4) are not in the client model before
        // the save, and come back stored (mode 0), not in the belt.
        run.findings.push(format!(
            "q-fix-real-start-belt: items before the save {:?}, after the load {:?}",
            before.2, after.2
        ));
    }
    run.findings.extend(findings);
    run.no_findings();
}

impl Run {
    /// The hostile monsters (not allied, not dying or dead: the server's
    /// sides, `LocalSeams::sides`) the client model holds with a
    /// position, nearest to the server player first.
    fn monsters_near(&self) -> Vec<UnitKey> {
        let foes: Vec<(u32, (i32, i32))> = app_support::with(&self.server, |l| {
            let g = &mut l.host_mut().game;
            let lists = &g.game.lists;
            let x = &g.events.action.hooks().x;
            x.sides
                .iter()
                .filter(|(u, (ty, allied, _))| {
                    *ty == d2_sim::units::UnitType::Monster && !allied && !x.down.contains(u)
                })
                .filter_map(|(u, &(_, _, p))| Some((lists.unit(*u)?.guid, p)))
                .collect()
        });
        let w = self.app.world().resource::<BridgeResource>().0.world();
        let (px, py) = self.pos();
        let mut v: Vec<(i32, UnitKey)> = w
            .units
            .iter()
            .filter(|(k, u)| k.unit_type == 1 && u.position.is_some() && !u.is_dead())
            .filter_map(|(k, _)| {
                let &(_, (x, y)) = foes.iter().find(|(g, _)| *g == k.guid)?;
                Some(((x - px).abs().max((y - py).abs()), *k))
            })
            .collect();
        v.sort();
        v.into_iter().map(|(_, k)| k).collect()
    }

    /// The ground items of the model (`bridge::items::ground_items`).
    fn ground_items(&self) -> Vec<d2_client::bridge::items::ItemView> {
        let w = self.app.world().resource::<BridgeResource>().0.world();
        d2_client::bridge::items::ground_items(w)
    }

    /// What a save keeps of the server player: level, experience, the
    /// stats, gold, its class skills' levels, its items (code, mode,
    /// body location), sorted, its waypoints and its quest flags.
    fn snapshot(&self) -> Snapshot {
        let stats = [0u16, 1, 2, 3, 4, 5, 12, 13, 14]
            .iter()
            .map(|&s| (s, self.player_stat(s)))
            .collect();
        let skills = app_support::with(&self.server, |l| {
            let g = &mut l.host_mut().game;
            let (p, _) = single_player::local_player(g).unwrap();
            let h = g.events.action.hooks();
            let mut v: Vec<(u16, i32)> = h
                .skill_lists
                .get(&p)
                .map(|l| l.entries.iter().map(|e| (e.skill, e.base)).collect())
                .unwrap_or_default();
            v.sort();
            v
        });
        let mut items: Vec<([u8; 4], u8, u8)> = self
            .local_items()
            .into_iter()
            .filter_map(|i| Some((i.code?, i.mode, i.body)))
            .collect();
        items.sort();
        let (waypoints, quests) = app_support::with(&self.server, |l| {
            let live = d2_client::app::save::read_live(&mut l.host_mut().game).unwrap();
            (live.extra.waypoints, live.quests.map(|q| q.to_vec()))
        });
        assert!(
            waypoints.is_some() && quests.is_some(),
            "the save reads both"
        );
        (stats, skills, items, waypoints, quests)
    }

    /// The local player's items (`bridge::items::local_items`).
    fn local_items(&self) -> Vec<d2_client::bridge::items::ItemView> {
        let w = self.app.world().resource::<BridgeResource>().0.world();
        d2_client::bridge::items::local_items(w)
    }

    /// The server player's base stat `s`.
    fn player_stat(&self, s: u16) -> i32 {
        app_support::with(&self.server, move |l| {
            let g = &l.host().game;
            let (p, _) = single_player::local_player(g).unwrap();
            g.events.action.sys.stats.unit_base(p, s, 0)
        })
    }

    /// The monster `guid` is dying or dead on the server (mode 0 or 12,
    /// `units.md`), or gone. Its life is not the test: a corpse's life
    /// still regenerates (`stat-lists.md` §10.1 has no dead-mode stop;
    /// open question in `docs/handoff/q-fixture-migrate.md`).
    fn monster_dead(&self, guid: u32) -> bool {
        app_support::with(&self.server, move |l| {
            let sim = &mut l.host_mut().game;
            let Some(m) = sim
                .game
                .lists
                .find_unit(d2_sim::units::UnitType::Monster, guid)
            else {
                return true;
            };
            sim.events
                .action
                .sys
                .units
                .get(m)
                .is_none_or(|r| matches!(r.mode, 0 | 12))
        })
    }

    /// The server's life of the monster `guid` (`None`: gone).
    fn monster_life(&self, guid: u32) -> Option<i32> {
        app_support::with(&self.server, move |l| {
            let sim = &mut l.host_mut().game;
            let m = sim
                .game
                .lists
                .find_unit(d2_sim::units::UnitType::Monster, guid)?;
            let a = &mut sim.events.action;
            Some(a.with(&mut sim.game, |_, v| {
                v.stat(m, d2_sim::stats::stat::HITPOINTS)
            }))
        })
    }

    /// A 15-sub-tile leg in the first of the eight directions that moves
    /// the player; `false` when none does.
    fn sidestep(&mut self) -> bool {
        let p = self.pos();
        for d in [
            (15, 0),
            (0, 15),
            (-15, 0),
            (0, -15),
            (15, 15),
            (-15, 15),
            (15, -15),
            (-15, -15),
        ] {
            self.leg((p.0 + d.0, p.1 + d.1));
            if test_fixtures::host::cheb(self.pos(), p) > 3 {
                return true;
            }
        }
        false
    }

    /// Left skill (Attack) on `key` (C→S 0x06) until the server says it
    /// is dead; one message each time the player is back in a neutral
    /// mode.
    /// `false`: out of reach (no leg toward it moves the player) or
    /// still alive.
    fn kill(&mut self, key: UnitKey) -> bool {
        for _ in 0..20 {
            if self.monster_dead(key.guid) {
                return true;
            }
            // A far monster: run toward it first (the unit target is
            // refused past its range, `intents-events.md` §2.4 r4).
            let mp = app_support::with(&self.server, move |l| {
                let g = &mut l.host_mut().game;
                let u = g
                    .game
                    .lists
                    .find_unit(d2_sim::units::UnitType::Monster, key.guid)?;
                Some(g.events.action.hooks().path_position(u))
            });
            if let Some(mp) = mp {
                let p = self.pos();
                if test_fixtures::host::cheb(p, mp) > 30 {
                    self.leg((
                        p.0 + (mp.0 - p.0).clamp(-25, 25),
                        p.1 + (mp.1 - p.1).clamp(-25, 25),
                    ));
                    // A blocked leg stays: step aside, else give the
                    // monster up (out of reach).
                    if test_fixtures::host::cheb(self.pos(), p) <= 3 && !self.sidestep() {
                        return false;
                    }
                    continue;
                }
            }
            let mut m = vec![0x06, 1, 0, 0, 0];
            m.extend_from_slice(&key.guid.to_le_bytes());
            self.bridge().send_bytes(&m).unwrap();
            let mut seen = vec![self.mode()];
            for _ in 0..200 {
                self.step(1);
                let m = self.mode();
                if seen.last() != Some(&m) {
                    seen.push(m);
                }
                if seen.len() > 1 && matches!(m, 1 | 5) {
                    break;
                }
            }
            self.check("attack");
            if std::env::var_os("SMOKE_DEBUG").is_some() {
                let (log, errs) = app_support::with(&self.server, |l| {
                    let h = l.host_mut().game.events.action.hooks();
                    (
                        h.x.skills
                            .log
                            .iter()
                            .rev()
                            .take(4)
                            .cloned()
                            .collect::<Vec<_>>(),
                        format!("{:?}", h.errors.iter().rev().take(3).collect::<Vec<_>>()),
                    )
                });
                let g = key.guid;
                let mmode = app_support::with(&self.server, move |l| {
                    let sim = &mut l.host_mut().game;
                    let m = sim
                        .game
                        .lists
                        .find_unit(d2_sim::units::UnitType::Monster, g)?;
                    let r = sim.events.action.sys.units.get(m)?;
                    Some((r.class, r.mode, r.flags))
                });
                eprintln!(
                    "attack {:?} {mmode:?}: modes {seen:?}, life {:?}, player {:?}, monster {mp:?}, log {log:?} errors {errs}",
                    key.guid,
                    self.monster_life(key.guid),
                    self.pos()
                );
            }
        }
        self.monster_dead(key.guid)
    }
}

/// A run into a wall: the drawn player stops where the server's player
/// stops. The fixture tiles have no walls, so the test stamps one (wall
/// bit 0x1, `drlg/rooms.md` §10) into the server's and the client's grids
/// alike, as 1.14d builds both from the same tiles. The walk prediction
/// steps the player's own path over the client grids (`ClientPath`,
/// REC-277 (d)); a straight line would run on through the wall.
#[test]
#[ignore = "real data: needs D2_GAME_DIR (tools/realdata-gate.sh)"]
fn a_blocked_run_is_drawn_where_the_server_stops() {
    let mut run = Run::start();
    let p = run.pos();
    // 12 sub-tiles east, 61 long: past the A* radius (`pathing.md` §7),
    // so neither path goes round it.
    let wall: Vec<(i32, i32)> = (-30..=30).map(|k| (p.0 + 12, p.1 + k)).collect();
    let w = wall.clone();
    let server = app_support::with(&run.server, move |l| {
        let g = &mut l.host_mut().game;
        let d = g.events.action.hooks().drlg.dungeon.acts[0]
            .as_mut()
            .expect("Act I");
        w.iter()
            .filter_map(|&(x, y)| d.collision_at_mut(x, y).map(|v| *v |= 1))
            .count()
    });
    let client = {
        let mut b = run.app.world_mut().resource_mut::<BridgeResource>();
        let d = &mut b.0.drlg_mut().expect("the client DRLG").drlg;
        wall.iter()
            .filter_map(|&(x, y)| d.collision_at_mut(x, y).map(|v| *v |= 1))
            .count()
    };
    assert_eq!(
        (server, client),
        (wall.len(), wall.len()),
        "the wall in both grids"
    );
    run.leg((p.0 + 24, p.1));
    let s = run.pos();
    assert!(
        s.0 > p.0 && s.0 < p.0 + 12,
        "the server stopped at the wall: {p:?} → {s:?}"
    );
    let drawn = run.drawn();
    assert!(
        test_fixtures::host::cheb(drawn, s) <= 2,
        "the drawn player stopped there too: {drawn:?} / {s:?}"
    );
}
