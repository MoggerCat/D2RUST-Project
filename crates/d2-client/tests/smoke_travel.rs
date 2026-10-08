// Spec: specs/world/waypoints.md (§6, §7), specs/world/objects.md (§12), specs/world/npc.md (§8.3), specs/sim/path-placement.md (§12.1, §12.2), specs/client/model.md (§8 rule 7, §11, §12, §16), specs/client/bridge.md (§6)
//! (q-smoke-travel) The travel flows of the app's synthetic game end to
//! end through the real play path (bridge + in-process server + sim, no
//! window): every waypoint of the five acts, Town Portal round trips,
//! the act travel the game allows (waypoint tabs, Warriv, Meshif, Tyrael,
//! Cain) and every level warp down and back up. After every move the
//! smoke checks hold ([`Rig::arrive`]): no intent the server refused, no
//! S→C message dropped, unowned, discarded or rejected by the client, the
//! client's level, room and position follow the server's, and the units
//! of the level left behind are gone from the model once their rooms are.
//!
//! Stand-ins (the synthetic world has no such objects; see
//! `docs/handoff/q-smoke-travel.md`): a field waypoint is learned by its
//! record bit, a first arrival at a level no travel reaches yet (and the
//! Act III → IV hop, no Mephisto portal) uses the act-change queue, and
//! the Town Portal is opened by the action the scroll's C→S 0x20 ends in.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use d2_client::app::play::{
    add_client_data, add_game, add_walk, predict_link, send_create_game_for,
};
use d2_client::app::server_thread::ThreadLink;
use d2_client::app::single_player::{self, GameData, WaypointTables};
use d2_client::app::synthetic_act4 as a4;
use d2_client::bridge::link::{LinkError, Pumped, SendQueue, Sent, ServerLink};
use d2_client::bridge::predict::Speeds;
use d2_client::bridge::world::{UnitKey, MONSTER, OBJECT, TILE};
use d2_client::bridge::BridgeResource;
use d2_server::dispatch::Outcome;
use d2_server::host::Handled;
use d2_server::seams::{Clock, ResultCode};
use d2_sim::units::UnitType;
use d2_sim::world::npc::class as npc;
use d2_sim::world::waypoints::NO_WAYPOINT;

mod app_support;
use app_support::SharedLink;

struct StepClock(Arc<AtomicU32>);

impl Clock for StepClock {
    fn now_ms(&mut self) -> u32 {
        self.0.load(Ordering::SeqCst)
    }
}

type Server = app_support::Server<StepClock>;

/// (type, GUID, class, position) of a model unit.
type UnitRow = (u8, u32, u32, Option<(u16, u16)>);

/// (level, destination level, tile class).
type Warp = (u32, u32, u32);

/// Per level: (destination level, tile class) of its warps.
type WarpsOut = BTreeMap<u32, Vec<(u32, u32)>>;

/// The shared link with the smoke audit: after every pump, each drained
/// C→S message the server did not take (`Outcome` other than a dispatch
/// with result 0) is noted; every S→C id is noted.
struct Audit {
    inner: SharedLink<ThreadLink<single_player::Link<StepClock>>>,
    server: Server,
    refused: Arc<Mutex<Vec<String>>>,
    seen: Arc<Mutex<Vec<u8>>>,
    log: Arc<Mutex<Vec<Vec<u8>>>>,
}

impl ServerLink for Audit {
    fn protocol_version(&self) -> u32 {
        self.inner.protocol_version()
    }
    fn send(&mut self, queue: SendQueue, msg: &[u8]) -> Result<Sent, LinkError> {
        self.inner.send(queue, msg)
    }
    fn pump(&mut self) -> Result<Pumped, LinkError> {
        let r = self.inner.pump();
        let msgs = app_support::with(&self.server, |l| l.last_frame().messages.clone());
        let mut refused = self.refused.lock().unwrap();
        for m in msgs {
            match m.handled {
                Handled::System | Handled::Game(Outcome::Dispatched(ResultCode::Done)) => {}
                other => refused.push(format!("C→S {:#04x}: {other:?}", m.id)),
            }
        }
        r
    }
    fn receive(&mut self) -> Vec<Vec<u8>> {
        let v = self.inner.receive();
        self.seen
            .lock()
            .unwrap()
            .extend(v.iter().filter_map(|m| m.first().copied()));
        self.log.lock().unwrap().extend(v.iter().cloned());
        v
    }
}

struct Rig {
    app: App,
    server: Server,
    ms: Arc<AtomicU32>,
    refused: Arc<Mutex<Vec<String>>>,
    seen: Arc<Mutex<Vec<u8>>>,
    log: Arc<Mutex<Vec<Vec<u8>>>>,
    steps: u32,
    /// What the run did, for the failure messages.
    trail: Vec<String>,
}

impl Rig {
    fn new() -> Rig {
        let data = GameData::Synthetic;
        let ms = Arc::new(AtomicU32::new(1000));
        let character = single_player::new_character("sorceress", "Test").unwrap();
        let (link, _) = single_player::start_with(
            data.clone(),
            single_player::DEFAULT_SEED,
            character.clone(),
            StepClock(ms.clone()),
        )
        .unwrap();
        let server: Server = Arc::new(Mutex::new(link));
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Image>()
            .init_resource::<ButtonInput<MouseButton>>();
        let refused = Arc::new(Mutex::new(Vec::new()));
        let seen = Arc::new(Mutex::new(Vec::new()));
        let log = Arc::new(Mutex::new(Vec::new()));
        let (link, tap) = predict_link(Box::new(Audit {
            inner: SharedLink(server.clone()),
            server: server.clone(),
            refused: refused.clone(),
            seen: seen.clone(),
            log: log.clone(),
        }));
        add_game(&mut app, link, false).unwrap();
        add_walk(&mut app, tap, Some(Speeds { walk: 6, run: 9 }));
        send_create_game_for(&mut app, &character).unwrap();
        add_client_data(
            &mut app,
            single_player::client_drlg_source(&data),
            single_player::client_level_rows(&data),
        );
        app_support::synthetic_skill_rows(&mut app);
        app.world_mut()
            .resource_mut::<BridgeResource>()
            .0
            .set_unit_rows(single_player::synthetic_unit_rows());
        app.update();
        let mut rig = Rig {
            app,
            server,
            ms,
            refused,
            seen,
            log,
            steps: 0,
            trail: Vec::new(),
        };
        while app_support::local_player(&rig.server).is_none() {
            rig.step(1);
        }
        rig.step(30);
        assert_eq!(rig.server_level(), Some(single_player::ACT1_TOWN));
        rig.check("the join");
        rig
    }

    fn step(&mut self, n: usize) {
        for _ in 0..n {
            self.ms.fetch_add(40, Ordering::SeqCst);
            self.app.update();
            self.steps += 1;
            assert!(self.steps < 400_000, "the run finishes");
        }
    }

    fn bridge(&self) -> &d2_client::bridge::Bridge<d2_client::bridge::mirror::DynLink> {
        &self.app.world().resource::<BridgeResource>().0
    }

    fn server_level(&self) -> Option<u32> {
        app_support::with(&self.server, |l| {
            let (p, _) = single_player::local_player(&l.host().game)?;
            let g = &mut l.host_mut().game;
            let room = g.game.lists.unit(p)?.room()?;
            g.events.action.hooks().drlg.level_id(&g.game, room)
        })
    }

    fn server_mode(&self) -> u32 {
        app_support::with(&self.server, |l| {
            let (p, _) = single_player::local_player(&l.host().game).unwrap();
            l.host()
                .game
                .events
                .action
                .sys
                .units
                .get(p)
                .map_or(99, |r| r.mode)
        })
    }

    /// The server player's position.
    fn server_position(&self) -> (i32, i32) {
        app_support::with(&self.server, |l| {
            let (p, _) = single_player::local_player(&l.host().game).unwrap();
            l.host_mut().game.events.action.hooks().path_position(p)
        })
    }

    /// Where the view draws the local player: the walk prediction's
    /// sub-tile.
    fn drawn(&self) -> Option<(u16, u16)> {
        self.app
            .world()
            .resource::<d2_client::world_view::walk::PreviewWalk>()
            .predict
            .cell()
    }

    /// A warp (`sim/path-placement.md` §12.2 r5) or portal
    /// (`world/objects.md` §12 r11) arrival walks the server player out
    /// to the 0x0D's target: the drawn player ends where it does (B3).
    fn drawn_on_server(&self, what: &str) {
        let (x, y) = self.server_position();
        let server = (u16::try_from(x).unwrap(), u16::try_from(y).unwrap());
        assert_eq!(
            self.drawn(),
            Some(server),
            "{what}: the drawn player stands on the server player; trail {:?}",
            self.trail
        );
    }

    fn client_level(&self) -> Option<u32> {
        self.bridge().world().player_level().map(u32::from)
    }

    /// The smoke checks of the bridge and the server so far.
    fn check(&self, at: &str) {
        let log = self.bridge().log();
        let refused = self.refused.lock().unwrap();
        assert!(refused.is_empty(), "{at}: server refused {refused:?}");
        assert!(log.rejected.is_empty(), "{at}: rejected {:?}", log.rejected);
        assert!(log.dropped.is_empty(), "{at}: dropped {:?}", log.dropped);
        assert!(log.unowned.is_empty(), "{at}: unowned {:?}", log.unowned);
        assert!(
            log.discarded.is_empty(),
            "{at}: discarded {:?}",
            log.discarded
        );
    }

    /// (type, GUID, class, position) of every unit of the model.
    fn unit_list(&self) -> Vec<UnitRow> {
        self.bridge()
            .world()
            .units
            .iter()
            .map(|(k, u)| (k.unit_type, k.guid, u.class, u.position))
            .collect()
    }

    fn find(&self, ty: u8, class: u32) -> Option<UnitKey> {
        self.bridge()
            .world()
            .units
            .iter()
            .find(|(k, u)| k.unit_type == ty && u.class == class)
            .map(|(k, _)| *k)
    }

    /// The portal of the player's own level. The other end of the pair
    /// can be in the model too, in a room of a neighbouring level in
    /// sight (the synthetic Spider Forest borders Kurast Docks), out of
    /// reach of a click (B4).
    fn portal_here(&self) -> Option<UnitKey> {
        let w = self.bridge().world();
        let level = w.player_level()?;
        w.units
            .iter()
            .filter(|(k, u)| {
                k.unit_type == OBJECT && u.class == single_player::SYNTHETIC_PORTAL_CLASS
            })
            .map(|(k, _)| *k)
            .find(|&k| w.unit_room(k).is_some_and(|r| r.level == level))
    }

    fn interact(&mut self, key: UnitKey) {
        let sent = self
            .app
            .world_mut()
            .resource_mut::<BridgeResource>()
            .0
            .interact(key)
            .unwrap();
        assert!(sent.is_empty(), "interact {key:?}: {sent:?}");
    }

    fn send(&mut self, m: &[u8]) {
        self.app
            .world_mut()
            .resource_mut::<BridgeResource>()
            .0
            .send_bytes(m)
            .unwrap();
    }

    /// The non-local units the model places in `level`.
    fn units_in(&self, level: u32) -> Vec<UnitKey> {
        let w = self.bridge().world();
        w.units
            .keys()
            .filter(|&&k| Some(k) != w.local_player)
            .filter(|&&k| w.unit_room(k).is_some_and(|r| u32::from(r.level) == level))
            .copied()
            .collect()
    }

    /// Runs `go`, then the arrival checks at `to`.
    fn travel(&mut self, what: &str, to: u32, go: impl FnOnce(&mut Rig)) {
        let from = self.server_level().expect("the player is placed");
        let old = self.units_in(from);
        self.trail.push(format!("{what}: {from} -> {to}"));
        self.log.lock().unwrap().clear();
        go(self);
        self.arrive(what, to, &old);
    }

    fn arrive(&mut self, what: &str, to: u32, old: &[UnitKey]) {
        let at = format!("{what} (to {to}; trail {:?})", self.trail);
        let mut n = 0;
        while self.server_level() != Some(to) {
            self.step(1);
            n += 1;
            assert!(n < 2000, "{at}: the server player reaches {to}");
        }
        // Until the server player stands (neutral 1, or 5 in town): a
        // portal arrival walks out (`objects.md` §12 r11).
        let mut n = 0;
        loop {
            self.step(1);
            n += 1;
            if n >= 40 && matches!(self.server_mode(), 1 | 5) {
                break;
            }
            assert!(n < 1000, "{at}: the server player stands");
        }
        self.step(5);
        assert_eq!(self.server_level(), Some(to), "{at}: stays");
        assert_eq!(self.client_level(), Some(to), "{at}: the client level");
        // The model moves a unit only when a message places it
        // (`client/model.md` §3 r3): the local player is where the last
        // S→C 0x15 / 0x59 of the move put it. (How the client moves its player
        // between messages, e.g. the arrival walk-outs, is model OQ2,
        // REC-51.)
        let me = self
            .bridge()
            .world()
            .local_player
            .expect("the local player");
        let placed = self
            .log
            .lock()
            .unwrap()
            .iter()
            .rev()
            .find_map(|m| match m[0] {
                // 0x15 ReassignPlayer: type @1, GUID @2, x @6, y @8.
                0x15 if m[1] == 0 && m[2..6] == me.guid.to_le_bytes() => Some((
                    u16::from_le_bytes([m[6], m[7]]),
                    u16::from_le_bytes([m[8], m[9]]),
                )),
                // 0x59 AssignPlayer (an act change): GUID @1, x @22, y @24.
                0x59 if m[1..5] == me.guid.to_le_bytes() => Some((
                    u16::from_le_bytes([m[22], m[23]]),
                    u16::from_le_bytes([m[24], m[25]]),
                )),
                _ => None,
            });
        let placed = placed.unwrap_or_else(|| panic!("{at}: an S→C 0x15 / 0x59 placed the player"));
        assert_eq!(
            self.bridge().world().units[&me].position,
            Some(placed),
            "{at}: the model position"
        );
        // The drawn player (the walk prediction) is where the arrival's
        // walk request took it: the last S→C 0x0D code 1 of the move
        // (player code 0x01, walk to (x, y), `client/model.md` §8 r4; the
        // warp, portal and waypoint walk-outs), else the placement (B3,
        // REC-288).
        let walk_out = self.log.lock().unwrap().iter().rev().find_map(|m| {
            // 0x0D: type @1, GUID @2, code @6, x @7, y @9.
            (m[0] == 0x0D && m[1] == 0 && m[2..6] == me.guid.to_le_bytes() && m[6] == 1).then(
                || {
                    (
                        u16::from_le_bytes([m[7], m[8]]),
                        u16::from_le_bytes([m[9], m[10]]),
                    )
                },
            )
        });
        assert_eq!(
            self.drawn(),
            Some(walk_out.unwrap_or(placed)),
            "{at}: the drawn player"
        );
        let w = self.bridge().world();
        let me = w.local_player.expect("the local player");
        let pos = w.units[&me].position.expect("the model position");
        let room = w.local_room().expect("the player's client room");
        assert!(
            room.contains(i32::from(pos.0), i32::from(pos.1)),
            "{at}: the client room {room:?} holds {pos:?}"
        );
        // The old level's units: still in an active room (in sight), or
        // gone from the model.
        let stale: Vec<_> = old
            .iter()
            .filter(|k| w.units.contains_key(k) && w.unit_room(**k).is_none())
            .collect();
        assert!(stale.is_empty(), "{at}: old units not freed {stale:?}");
        self.check(&at);
    }

    /// Stand-in: the act-change queue puts the player in `level`.
    fn put(&mut self, level: u32) {
        if self.server_level() == Some(level) {
            return;
        }
        self.travel("put", level, |r| {
            app_support::with(&r.server, move |l| {
                let (p, _) = single_player::local_player(&l.host().game).unwrap();
                l.host_mut()
                    .game
                    .events
                    .action
                    .hooks()
                    .act_changes
                    .push((p, level, 0));
            })
        });
    }

    fn knows_waypoint(&self, index: u8) -> bool {
        app_support::with(&self.server, move |l| {
            let (p, _) = single_player::local_player(&l.host().game).unwrap();
            let g = &mut l.host_mut().game;
            let wp = g.events.action.hooks().waypoints.entry(p).or_default();
            wp.get_mut(0).test(u32::from(index)).unwrap_or(false)
        })
    }

    /// Stand-in for a field waypoint the synthetic world has no object
    /// for: the record bit.
    fn learn_waypoint(&self, index: u8) {
        app_support::with(&self.server, move |l| {
            let (p, _) = single_player::local_player(&l.host().game).unwrap();
            let g = &mut l.host_mut().game;
            let wp = g.events.action.hooks().waypoints.entry(p).or_default();
            wp.get_mut(0).set(u32::from(index)).unwrap();
        });
    }

    /// Walks next to `key` the way a ground click does (C→S 0x01 to the
    /// unit's sub-tile, `ui/controls.md` §6 r7) and waits until the
    /// server player stands.
    fn approach(&mut self, key: UnitKey) {
        let (x, y) = self.bridge().world().units[&key]
            .position
            .expect("the unit's position");
        let mut m = vec![0x01];
        m.extend_from_slice(&x.to_le_bytes());
        m.extend_from_slice(&y.to_le_bytes());
        self.send(&m);
        let mut n = 0;
        loop {
            self.step(1);
            n += 1;
            if n >= 5 && matches!(self.server_mode(), 1 | 5) {
                break;
            }
            assert!(n < 1500, "walking to {key:?}; trail {:?}", self.trail);
        }
        self.check("approach");
    }

    /// Walks to the waypoint object in sight and operates it (C→S 0x13):
    /// the menu (S→C 0x63) opens.
    fn operate_waypoint(&mut self) -> UnitKey {
        let wp = self.find(OBJECT, 0).unwrap_or_else(|| {
            panic!(
                "a waypoint in sight; trail {:?}; {:?}",
                self.trail,
                self.unit_list()
            )
        });
        self.approach(wp);
        self.seen.lock().unwrap().clear();
        self.interact(wp);
        let mut n = 0;
        while !self.seen.lock().unwrap().contains(&0x63) {
            self.step(1);
            n += 1;
            assert!(
                n < 400,
                "the waypoint menu opens; trail {:?}; refused {:?}; seen {:?}",
                self.trail,
                self.refused.lock().unwrap(),
                self.seen.lock().unwrap()
            );
        }
        self.step(5);
        self.check("operate waypoint");
        wp
    }

    /// Closes the open waypoint menu (C→S 0x49 level 0, the close hook
    /// `0x0049CF50`, `ui/panels.md` §13.7).
    fn close_waypoint(&mut self, wp: UnitKey) {
        let mut m = vec![0x49];
        m.extend_from_slice(&wp.guid.to_le_bytes());
        m.extend_from_slice(&0u32.to_le_bytes());
        self.send(&m);
        self.step(5);
        self.check("close waypoint");
    }

    /// Operates the waypoint in sight and takes it to `to` (C→S 0x49).
    fn take_waypoint(&mut self, to: u32) {
        self.travel("waypoint", to, |r| {
            let wp = r.operate_waypoint();
            let mut m = vec![0x49];
            m.extend_from_slice(&wp.guid.to_le_bytes());
            m.extend_from_slice(&to.to_le_bytes());
            r.send(&m);
        });
    }

    fn set_quest_flag(&self, slot: u8, bit: u8) {
        app_support::with(&self.server, move |l| {
            let sim = &mut l.host_mut().game;
            let (p, _) = single_player::local_player(sim).unwrap();
            sim.world.rest.quests.entry(p).or_default().flags[0].set(slot, bit);
        });
    }

    /// Talks to the town NPC of `class` (C→S 0x13), then picks its travel
    /// row (C→S 0x38 [action][GUID][0]).
    fn npc_travel(&mut self, what: &str, class: u16, to: u32) {
        let key = self.find(MONSTER, u32::from(class)).unwrap_or_else(|| {
            let units: Vec<_> = self
                .bridge()
                .world()
                .units
                .iter()
                .map(|(k, u)| (k.unit_type, u.class, u.position))
                .collect();
            panic!("{what}: NPC {class} in sight; {units:?}")
        });
        self.travel(what, to, |r| {
            r.approach(key);
            r.seen.lock().unwrap().clear();
            r.interact(key);
            let mut n = 0;
            while !r.seen.lock().unwrap().contains(&0x28) {
                r.step(1);
                n += 1;
                assert!(n < 600, "{what}: the chat opens");
            }
            r.step(5);
            let mut m = vec![0x38];
            m.extend_from_slice(&0u32.to_le_bytes());
            m.extend_from_slice(&key.guid.to_le_bytes());
            m.extend_from_slice(&0u32.to_le_bytes());
            r.send(&m);
        });
        let act = d2_sim::drlg::act_of_level(to);
        let w = self.bridge().world();
        assert_eq!(w.act.as_ref().map(|a| a.act), Some(act), "{what}: act");
        assert!(w.in_game, "{what}: in the game again");
    }
}

/// (level, waypoint index, act) of every synthetic waypoint level.
fn waypoint_levels() -> Vec<(u32, u8, u8)> {
    WaypointTables::synthetic()
        .levels
        .iter()
        .enumerate()
        .filter(|(_, l)| l.waypoint != NO_WAYPOINT)
        .map(|(i, l)| (i as u32, l.waypoint, l.act))
        .collect()
}

const TOWNS: [u32; 5] = [
    single_player::ACT1_TOWN,
    single_player::ACT2_TOWN,
    75,
    a4::FORTRESS,
    single_player::ACT5_TOWN,
];

/// The act's town waypoint is activated by its use, then every field
/// waypoint of the act is taken from it (a fresh game per act: B2 in
/// `docs/handoff/q-smoke-travel.md`).
fn waypoints_of_act(act: usize) {
    let mut rig = Rig::new();
    let wps = waypoint_levels();
    let town = TOWNS[act];
    rig.put(town);
    let (_, index, _) = *wps.iter().find(|w| w.0 == town).expect("a town waypoint");
    let wp = rig.operate_waypoint();
    assert!(rig.knows_waypoint(index), "act {act}: activated");
    rig.close_waypoint(wp);
    for &(level, index, _) in wps.iter().filter(|w| w.2 as usize == act && w.0 != town) {
        rig.learn_waypoint(index);
        rig.take_waypoint(level);
        rig.put(town);
    }
}

// Covers: specs/world/waypoints.md §6.3 r2; specs/world/waypoints.md §7 r5; specs/client/model.md §11
#[test]
fn the_act_i_waypoints_activate_and_take_the_player() {
    waypoints_of_act(0);
}

// Covers: specs/world/waypoints.md §6.3 r2; specs/world/waypoints.md §7 r5
#[test]
fn the_act_ii_waypoint_activates() {
    waypoints_of_act(1);
}

// Covers: specs/world/waypoints.md §6.3 r2; specs/world/waypoints.md §7 r5
#[test]
fn the_act_iii_waypoints_activate_and_take_the_player() {
    waypoints_of_act(2);
}

// Covers: specs/world/waypoints.md §6.3 r2; specs/world/waypoints.md §7 r5
#[test]
fn the_act_iv_waypoint_activates() {
    waypoints_of_act(3);
}

// Covers: specs/world/waypoints.md §6.3 r2; specs/world/waypoints.md §7 r5
#[test]
fn the_act_v_waypoints_activate_and_take_the_player() {
    waypoints_of_act(4);
}

/// The act tabs: from `from`'s waypoint (activated by its use) to the
/// town `to` (its bit learned: the town is not visited first, B2).
fn tab(from: usize, to: usize) {
    let mut rig = Rig::new();
    let wps = waypoint_levels();
    rig.put(TOWNS[from]);
    let (_, index, _) = *wps
        .iter()
        .find(|w| w.0 == TOWNS[to])
        .expect("a town waypoint");
    rig.learn_waypoint(index);
    rig.take_waypoint(TOWNS[to]);
}

// Covers: specs/world/waypoints.md §7 r5; specs/client/model.md §11
#[test]
fn the_waypoint_tabs_take_the_player_to_the_next_act() {
    for act in 0..4 {
        tab(act, act + 1);
    }
}

// Covers: specs/world/waypoints.md §7 r5; specs/client/model.md §11
#[test]
fn the_waypoint_tabs_take_the_player_back_to_the_act_before() {
    for act in 1..5 {
        tab(act, act - 1);
    }
}

/// B2 (`docs/handoff/q-smoke-travel.md`, fixed by q-fix-idle-rooms): a
/// town the player has not been to keeps its NPCs and waypoint however
/// long the game ran. The server frees the idle town room; the inactive
/// store keeps its units' records and the room's next build restores
/// them.
// Covers: specs/sim/units.md §3.3; specs/sim/units.md §3.4
#[test]
fn a_town_keeps_its_npcs_and_waypoint_after_a_long_game() {
    let mut rig = Rig::new();
    rig.put(single_player::COLD_PLAINS);
    rig.put(single_player::ACT1_TOWN);
    rig.put(single_player::COLD_PLAINS);
    rig.put(single_player::ACT2_TOWN);
    assert!(rig.find(OBJECT, 0).is_some(), "the Act II waypoint");
    assert!(
        rig.find(MONSTER, u32::from(npc::WARRIV2)).is_some(),
        "Warriv"
    );
}

/// A server unit of a level: (type, class, GUID, position, mode).
type ServerUnit = (UnitType, u32, u32, (i32, i32), u32);

/// The monsters and objects the server has in `level`'s active rooms.
fn server_units(rig: &Rig, level: u32) -> Vec<ServerUnit> {
    app_support::with(&rig.server, move |l| {
        let g = &l.host().game;
        let sys = &g.events.action.sys;
        let mut out = Vec::new();
        for ty in [UnitType::Monster, UnitType::Object] {
            for u in g.game.lists.units_of_type(ty) {
                let Some(room) = g.game.lists.unit(u).and_then(|e| e.room()) else {
                    continue;
                };
                if sys.hooks.drlg.level_id(&g.game, room) != Some(level) {
                    continue;
                }
                let r = sys.units.get(u).unwrap();
                out.push((ty, r.class, r.guid, sys.hooks.path_position(u), r.mode));
            }
        }
        out.sort();
        out
    })
}

/// The active rooms of `level` on the server.
fn server_rooms(rig: &Rig, act: u8, level: u32) -> usize {
    app_support::with(&rig.server, move |l| {
        let g = &l.host().game;
        let rooms = g.game.lists.active_rooms(act);
        let hooks = &g.events.action.sys.hooks;
        rooms
            .into_iter()
            .filter(|&r| hooks.drlg.level_id(&g.game, r) == Some(level))
            .count()
    })
}

/// The monster records (class, GUID, x, y) and other records (type,
/// class, x, y) the inactive store holds for `act`.
#[allow(clippy::type_complexity)]
fn stored(rig: &Rig, act: u8) -> (Vec<(u32, u32, i32, i32)>, Vec<(u8, u32, i32, i32)>) {
    app_support::with(&rig.server, move |l| {
        let g = &l.host().game;
        let store = g
            .events
            .action
            .sys
            .hooks
            .inactive
            .as_ref()
            .expect("the store is on");
        let nodes = &store.acts[usize::from(act)];
        let mut m: Vec<_> = nodes
            .iter()
            .flat_map(|n| n.monsters.iter().map(|r| (r.class, r.guid, r.x, r.y)))
            .collect();
        let mut o: Vec<_> = nodes
            .iter()
            .flat_map(|n| n.others.iter().map(|r| (r.ty, r.class, r.x, r.y)))
            .collect();
        m.sort();
        o.sort();
        (m, o)
    })
}

/// B2 fixed: the town is left, the game runs until the server frees every
/// town room (its units go to the inactive store, `units.md` §3.3), and
/// the player comes back: each NPC is spawned again with its GUID at the
/// place it was stored, the waypoint is a new object at its place
/// (§3.4 rule 4), the client sees them, the waypoint still operates and
/// Warriv still takes the player east.
// Covers: specs/sim/units.md §3.3; specs/sim/units.md §3.4; specs/drlg/rooms.md §8
#[test]
fn the_act_i_town_comes_back_with_its_npcs_and_waypoint() {
    let town = single_player::ACT1_TOWN;
    let mut rig = Rig::new();
    let before = server_units(&rig, town);
    let classes = |v: &[ServerUnit]| -> Vec<(UnitType, u32)> {
        let mut c: Vec<_> = v.iter().map(|u| (u.0, u.1)).collect();
        c.sort();
        c
    };
    for &(class, _) in &d2_client::app::town_npcs::ACT1 {
        assert!(
            before
                .iter()
                .any(|u| u.0 == UnitType::Monster && u.1 == u32::from(class)),
            "NPC {class} in town at the start"
        );
    }
    assert!(
        before.iter().any(|u| u.0 == UnitType::Object && u.1 == 0),
        "the waypoint"
    );
    rig.put(single_player::COLD_PLAINS);
    let mut n = 0;
    while server_rooms(&rig, 0, town) > 0 {
        rig.step(1);
        n += 1;
        assert!(n < 20_000, "the idle town's rooms are freed");
    }
    assert!(server_units(&rig, town).is_empty());
    let (monsters, others) = stored(&rig, 0);
    for u in before.iter().filter(|u| u.0 == UnitType::Monster) {
        assert!(
            monsters.iter().any(|m| (m.0, m.1) == (u.1, u.2)),
            "NPC {} (GUID {}) stored; {monsters:?}",
            u.1,
            u.2
        );
    }
    assert!(
        others.iter().any(|o| (o.0, o.1) == (2, 0)),
        "the waypoint stored; {others:?}"
    );
    rig.put(town);
    let after = server_units(&rig, town);
    assert_eq!(classes(&after), classes(&before), "the town's units");
    // The town's records (the act's store also holds the other idle
    // levels' units, e.g. the Black Marsh's tome).
    let guids: Vec<u32> = before
        .iter()
        .filter(|u| u.0 == UnitType::Monster)
        .map(|u| u.2)
        .collect();
    for m in monsters.iter().filter(|m| guids.contains(&m.1)) {
        assert!(
            after
                .iter()
                .any(|u| (u.0, u.1, u.2, u.3) == (UnitType::Monster, m.0, m.1, (m.2, m.3))),
            "NPC {m:?} restored with its GUID and place; {after:?}"
        );
    }
    for o in before.iter().filter(|u| u.0 == UnitType::Object) {
        assert!(
            others
                .iter()
                .any(|r| (r.0, r.1, (r.2, r.3)) == (2, o.1, o.3)),
            "object {o:?} stored at its place; {others:?}"
        );
        assert!(
            after
                .iter()
                .any(|u| (u.0, u.1, u.3, u.4) == (UnitType::Object, o.1, o.3, o.4)),
            "object {o:?} restored at its place in its mode; {after:?}"
        );
    }
    assert!(
        !stored(&rig, 0).0.iter().any(|m| guids.contains(&m.1)),
        "the town's records are used"
    );
    for &(class, _) in &d2_client::app::town_npcs::ACT1 {
        assert!(
            rig.find(MONSTER, u32::from(class)).is_some(),
            "the client sees NPC {class}"
        );
    }
    let wp = rig.operate_waypoint();
    rig.close_waypoint(wp);
    // Sisters to the Slaughter done (slot 6 bit 0).
    rig.set_quest_flag(6, 0);
    rig.npc_travel("Warriv east", npc::WARRIV1, single_player::ACT2_TOWN);
}

/// The class-59 objects of the server's game.
fn server_portals(rig: &Rig) -> usize {
    app_support::with(&rig.server, |l| {
        let g = &l.host().game;
        g.game
            .lists
            .units_of_type(UnitType::Object)
            .into_iter()
            .filter(|&u| {
                g.events
                    .action
                    .sys
                    .units
                    .get(u)
                    .is_some_and(|r| r.class == single_player::SYNTHETIC_PORTAL_CLASS)
            })
            .count()
    })
}

/// A Town Portal in the act's first field waypoint level: to town and
/// back, and the pair goes (a fresh game per act: B2).
fn town_portal_in_act(act: usize) {
    let mut rig = Rig::new();
    let wps = waypoint_levels();
    let town = TOWNS[act];
    let &(field, index, _) = wps
        .iter()
        .find(|w| w.2 as usize == act && w.0 != town)
        .expect("a field waypoint");
    rig.put(town);
    rig.learn_waypoint(index);
    rig.take_waypoint(field);
    let made = app_support::with(&rig.server, |l| {
        let (p, _) = single_player::local_player(&l.host().game).unwrap();
        let g = &mut l.host_mut().game;
        g.events.action.open_town_portal(&mut g.game, p).is_some()
    });
    assert!(made, "act {act}: the pair");
    rig.step(10);
    rig.check("the portal pair");
    let portal = rig.portal_here().expect("the field portal in the model");
    // The portal's hostile delay (`objects.md` §12 rule 2).
    rig.ms.fetch_add(10_000, Ordering::SeqCst);
    rig.travel("to town", town, |r| r.interact(portal));
    rig.drawn_on_server("to town");
    let portal = rig.portal_here().expect("the town portal in the model");
    rig.travel("back", field, |r| r.interact(portal));
    rig.drawn_on_server("back");
    assert_eq!(server_portals(&rig), 0, "act {act}: the pair went");
    assert!(rig
        .find(OBJECT, single_player::SYNTHETIC_PORTAL_CLASS)
        .is_none());
}

// Covers: specs/world/objects.md §12 r6; specs/world/objects.md §12 r8; specs/world/objects.md §12 r12
#[test]
fn a_town_portal_in_act_i_goes_to_town_and_back() {
    town_portal_in_act(0);
}

// Covers: specs/world/objects.md §12 r6; specs/world/objects.md §12 r12
#[test]
fn a_town_portal_in_act_iii_goes_to_town_and_back() {
    town_portal_in_act(2);
}

// Covers: specs/world/objects.md §12 r6; specs/world/objects.md §12 r12
#[test]
fn a_town_portal_in_act_v_goes_to_town_and_back() {
    town_portal_in_act(4);
}

/// A fresh game in `town` (B2), the quest flag set, `npc`'s travel
/// row taken to `to`.
fn npc_hop(town: u32, flag: Option<(u8, u8)>, what: &str, npc: u16, to: u32) {
    let mut rig = Rig::new();
    rig.put(town);
    if let Some((slot, bit)) = flag {
        rig.set_quest_flag(slot, bit);
    }
    rig.npc_travel(what, npc, to);
}

// Covers: specs/world/npc.md §8.3; specs/world/waypoints.md §7 r5; specs/client/model.md §11
#[test]
fn warriv_takes_the_player_east_after_sisters_to_the_slaughter() {
    // Sisters to the Slaughter done (slot 6 bit 0).
    npc_hop(
        single_player::ACT1_TOWN,
        Some((6, 0)),
        "Warriv east",
        npc::WARRIV1,
        single_player::ACT2_TOWN,
    );
}

// Covers: specs/world/npc.md §8.3
#[test]
fn warriv_takes_the_player_back_west() {
    npc_hop(
        single_player::ACT2_TOWN,
        None,
        "Warriv west",
        npc::WARRIV2,
        single_player::ACT1_TOWN,
    );
}

// Covers: specs/world/npc.md §8.3
#[test]
fn meshif_sails_east_after_the_seven_tombs() {
    // The Seven Tombs done (slot 14 bit 0).
    npc_hop(
        single_player::ACT2_TOWN,
        Some((14, 0)),
        "Meshif east",
        npc::MESHIF1,
        75,
    );
}

// Covers: specs/world/npc.md §8.3
#[test]
fn meshif_sails_back_west() {
    npc_hop(
        75,
        None,
        "Meshif west",
        npc::MESHIF2,
        single_player::ACT2_TOWN,
    );
}

// Covers: specs/world/npc.md §8.3
#[test]
fn tyrael_takes_the_player_to_harrogath_after_terrors_end() {
    // Terror's End done (slot 26 bit 0), an expansion game.
    npc_hop(
        a4::FORTRESS,
        Some((26, 0)),
        "Tyrael",
        npc::TYRAEL2,
        single_player::ACT5_TOWN,
    );
}

// Covers: specs/world/npc.md §8.3
#[test]
fn cain_takes_the_player_back_to_the_fortress() {
    npc_hop(
        single_player::ACT5_TOWN,
        None,
        "Cain back",
        npc::CAIN6,
        a4::FORTRESS,
    );
}

/// The warps of the synthetic game (Duriel's Lair is quest-gated,
/// `app_act2_duriel`'s), by level.
fn warp_graph() -> (Vec<Warp>, WarpsOut) {
    let lair = d2_client::app::synthetic_act2::DURIELS_LAIR;
    let warps: Vec<Warp> = single_player::synthetic_level_warps()
        .into_iter()
        .filter(|w| w.0 != lair && w.1 != lair)
        .collect();
    let mut out = WarpsOut::new();
    for &(from, to, class) in &warps {
        out.entry(from).or_default().push((to, class));
    }
    (warps, out)
}

/// Every warp reachable from `entry`, down and back up, in a fresh game
/// (B2); the warps taken are added to `used`.
fn walk_from(entry: u32, used: &mut BTreeSet<(u32, u32)>) {
    let (_, out) = warp_graph();
    let mut rig = Rig::new();
    let mut visited = BTreeSet::new();
    rig.put(entry);
    walk(&mut rig, &out, entry, &mut visited, used);
}

/// The levels the player reaches without a warp tile, per act.
const ENTRIES: [&[u32]; 5] = [
    &[
        single_player::COLD_PLAINS,
        single_player::STONY_FIELD,
        single_player::BLOOD_MOOR,
    ],
    &[single_player::ACT2_TOWN],
    &[75],
    &[a4::FORTRESS],
    &[single_player::ACT5_TOWN],
];

fn warps_of_act(act: usize) {
    let (warps, _) = warp_graph();
    let mut used = BTreeSet::new();
    for &entry in ENTRIES[act] {
        walk_from(entry, &mut used);
    }
    let missed: Vec<_> = warps
        .iter()
        .filter(|w| d2_sim::drlg::act_of_level(w.0) as usize == act)
        .filter(|w| !used.contains(&(w.0, w.2)))
        .collect();
    assert!(missed.is_empty(), "act {act}: warps not taken: {missed:?}");
}

// Covers: specs/sim/path-placement.md §12.2 r1; specs/sim/path-placement.md §12.2 r5
#[test]
fn every_act_i_warp_goes_down_and_back_up() {
    warps_of_act(0);
}

// Covers: specs/sim/path-placement.md §12.2 r1; specs/sim/path-placement.md §12.2 r5
#[test]
fn every_act_ii_warp_goes_down_and_back_up() {
    warps_of_act(1);
}

// Covers: specs/sim/path-placement.md §12.2 r1; specs/sim/path-placement.md §12.2 r5
#[test]
fn every_act_iii_warp_goes_down_and_back_up() {
    warps_of_act(2);
}

// Covers: specs/sim/path-placement.md §12.2 r1; specs/sim/path-placement.md §12.2 r5
#[test]
fn every_act_iv_warp_goes_down_and_back_up() {
    warps_of_act(3);
}

// Covers: specs/sim/path-placement.md §12.2 r1; specs/sim/path-placement.md §12.2 r5
#[test]
fn every_act_v_warp_goes_down_and_back_up() {
    warps_of_act(4);
}

/// Depth first from `level` (the player stands in it): every warp down,
/// the child's own warps, then the child's warp back up.
fn walk(
    rig: &mut Rig,
    out: &BTreeMap<u32, Vec<(u32, u32)>>,
    level: u32,
    visited: &mut BTreeSet<u32>,
    used: &mut BTreeSet<(u32, u32)>,
) {
    visited.insert(level);
    for &(to, class) in out.get(&level).map(Vec::as_slice).unwrap_or(&[]) {
        if used.contains(&(level, class)) {
            continue;
        }
        take_tile(rig, level, to, class);
        used.insert((level, class));
        if !visited.contains(&to) {
            walk(rig, out, to, visited, used);
        }
        // Back up the way the child leads to this level.
        let back = out
            .get(&to)
            .and_then(|v| v.iter().find(|w| w.0 == level))
            .map(|w| w.1);
        match back {
            Some(back) => {
                take_tile(rig, to, level, back);
                used.insert((to, back));
            }
            None => rig.put(level),
        }
    }
}

fn take_tile(rig: &mut Rig, level: u32, to: u32, class: u32) {
    assert_eq!(rig.server_level(), Some(level));
    let tile = rig
        .find(TILE, class)
        .unwrap_or_else(|| panic!("level {level}: tile {class} (to {to}) in the model"));
    rig.travel(&format!("tile {class}"), to, |r| r.interact(tile));
    rig.drawn_on_server(&format!("tile {class} to {to}"));
}
