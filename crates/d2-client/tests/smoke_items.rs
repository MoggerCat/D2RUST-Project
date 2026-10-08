// Spec: specs/items/inventory-moves.md (§7, §8, §11), specs/items/treasure.md (§3, §4), specs/client/msg-stats-items.md (§2), specs/client/stat-lists.md (§2)
//! (q-smoke-items) The item flows of the play game end to end, headless:
//! the app's synthetic game on its server thread, the bridge and the
//! client model, no window. Monster and chest drops reach the ground in
//! the client; the player picks items and gold up, moves them between
//! the grid, the belt, the stash, the cube and the cursor, drops them,
//! equips and swaps weapons, sockets a gem, carries a charm, identifies
//! and uses potions and scrolls. After every step: no intent the server
//! refused, no S→C message the client dropped or had no handler for, and
//! the client's items equal the server's item store.
//!
//! The item rows are the synthetic game's smoke items
//! (`app::synthetic_items::smoke`, PROVISIONAL REC-281, `d2rs-own,
//! unverified`).

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use d2_client::app::play::{
    add_client_data, add_game, add_walk, predict_link, send_create_game_for,
};
use d2_client::app::server_thread::ThreadLink;
use d2_client::app::single_player::{self, GameData};
use d2_client::app::synthetic_items::smoke;
use d2_client::bridge::items::{self, mode};
use d2_client::bridge::link::{LinkError, Pumped, SendQueue, Sent, ServerLink};
use d2_client::bridge::predict::Speeds;
use d2_client::bridge::world::{UnitKey, OBJECT};
use d2_client::bridge::BridgeResource;
use d2_server::dispatch::Outcome;
use d2_server::host::Handled;
use d2_server::seams::{Clock, ResultCode};
use d2_sim::units::lifecycle::AllocRequest;
use d2_sim::units::{UnitId, UnitType};

mod app_support;
use app_support::SharedLink;

struct StepClock(Arc<AtomicU32>);

impl Clock for StepClock {
    fn now_ms(&mut self) -> u32 {
        self.0.load(Ordering::SeqCst)
    }
}

type Server = app_support::Server<StepClock>;

/// Wraps the shared link and notes every C→S message the server's
/// dispatch did not finish (`ResultCode` other than done).
struct Tap {
    inner: SharedLink<ThreadLink<single_player::Link<StepClock>>>,
    refused: Arc<Mutex<Vec<(u8, ResultCode)>>>,
}

impl ServerLink for Tap {
    fn protocol_version(&self) -> u32 {
        self.inner.protocol_version()
    }
    fn send(&mut self, queue: SendQueue, msg: &[u8]) -> Result<Sent, LinkError> {
        self.inner.send(queue, msg)
    }
    fn pump(&mut self) -> Result<Pumped, LinkError> {
        let p = self.inner.pump()?;
        let refused = self
            .inner
            .0
            .lock()
            .unwrap()
            .with(|l| {
                l.last_frame()
                    .messages
                    .iter()
                    .filter_map(|m| match m.handled {
                        Handled::Game(Outcome::Dispatched(r)) if r != ResultCode::Done => {
                            Some((m.id, r))
                        }
                        _ => None,
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap();
        self.refused.lock().unwrap().extend(refused);
        Ok(p)
    }
    fn receive(&mut self) -> Vec<Vec<u8>> {
        self.inner.receive()
    }
}

/// Where an item is, as both sides can say it: (mode, body location,
/// page, x, y). Ground items: mode 3 and their sub-tile.
type Place = (u8, u8, u8, i32, i32);

struct Rig {
    app: App,
    server: Server,
    ms: Arc<AtomicU32>,
    refused: Arc<Mutex<Vec<(u8, ResultCode)>>>,
    steps: u32,
    /// The receive log's counts after the join (the base the steps are
    /// checked against).
    base: (BTreeMap<u8, u64>, BTreeMap<u8, u64>, usize, usize),
}

/// The chests' sub-tiles in the town room: beside the player's start
/// (23, 23), so each opens without a walk and its drop (the chest + (2,
/// 3), `treasure.md` §7 rule 2) lies within the pick-up reach (< 5).
/// The player never moves in these tests: a walk brings the walk-verify
/// S→C 0x96, which the client rejects for want of the visibility
/// predicate (`model.md` open question 7; not an item flow).
const CHESTS: [(i32, i32); 3] = [
    (single_player::WAYPOINT_X + 2, single_player::UNIT_Y),
    (single_player::WAYPOINT_X + 4, single_player::UNIT_Y),
    (single_player::WAYPOINT_X + 5, single_player::UNIT_Y + 1),
];
/// The stash's sub-tile.
const STASH: (i32, i32) = (single_player::WAYPOINT_X - 6, single_player::UNIT_Y + 3);

impl Rig {
    fn new() -> Rig {
        let data = GameData::Synthetic;
        let ms = Arc::new(AtomicU32::new(1000));
        let character = single_player::new_character("sorceress", "Test").unwrap();
        let (link, _) = single_player::start_with_objects(
            data.clone(),
            single_player::DEFAULT_SEED,
            character.clone(),
            StepClock(ms.clone()),
            CHESTS.to_vec(),
            Some(STASH),
        )
        .unwrap();
        let server: Server = Arc::new(Mutex::new(link));
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Image>()
            .init_resource::<ButtonInput<MouseButton>>();
        let refused = Arc::new(Mutex::new(Vec::new()));
        let (link, tap) = predict_link(Box::new(Tap {
            inner: SharedLink(server.clone()),
            refused: refused.clone(),
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
            steps: 0,
            base: Default::default(),
        };
        while app_support::local_player(&rig.server).is_none() {
            rig.step(1);
        }
        rig.step(30);
        // The synthetic `charstats` give no start stats: the base stats a
        // new sorceress starts with (as `app_a4_endgame.rs`).
        rig.set_base(0, 10); // strength
        rig.set_base(2, 25); // dexterity
        rig.set_base(d2_sim::items::stat::LEVEL, 1);
        rig.set_base(6, 40 << 8); // life
        rig.set_base(7, 40 << 8); // max life
        rig.step(30);
        let log = rig.bridge().log();
        rig.base = (
            log.unowned.clone(),
            log.dropped.clone(),
            log.rejected.len(),
            log.discarded.len(),
        );
        rig.check("join");
        rig
    }

    fn bridge(&self) -> &d2_client::bridge::Bridge<d2_client::bridge::mirror::DynLink> {
        &self.app.world().resource::<BridgeResource>().0
    }

    fn step(&mut self, n: usize) {
        for _ in 0..n {
            self.ms.fetch_add(40, Ordering::SeqCst);
            self.app.update();
            self.steps += 1;
            assert!(self.steps < 20000, "the test finishes");
        }
    }

    fn send_bytes(&mut self, m: &[u8]) {
        self.app
            .world_mut()
            .resource_mut::<BridgeResource>()
            .0
            .send_bytes(m)
            .unwrap();
    }

    fn send<M: d2_proto::wire::FixedMessage>(&mut self, m: &M) {
        self.app
            .world_mut()
            .resource_mut::<BridgeResource>()
            .0
            .send(m)
            .unwrap();
    }

    /// Sends `m` and runs frames until the server answered.
    fn act<M: d2_proto::wire::FixedMessage>(&mut self, what: &str, m: &M) {
        self.send(m);
        self.step(8);
        self.check(what);
    }

    fn act_bytes(&mut self, what: &str, m: &[u8]) {
        self.send_bytes(m);
        self.step(8);
        self.check(what);
    }

    /// The server's item store: GUID → (code, place) of every item that
    /// is on the ground or the local player's (its inventory, cursor and
    /// the fillers of its items).
    fn server_items(&self) -> BTreeMap<u32, ([u8; 4], Place)> {
        app_support::with(&self.server, |l| {
            let g = &mut l.host_mut().game;
            let (p, _) = single_player::local_player(g).unwrap();
            let mut out = BTreeMap::new();
            for u in g.game.lists.units_of_type(UnitType::Item) {
                let Some(rec) = g.events.action.hooks().items.get(u).map(|i| i.record) else {
                    continue;
                };
                let code = g.world.tables.item(rec).map(|r| r.code).unwrap_or([0; 4]);
                let Some(e) = g.game.lists.unit(u) else {
                    continue;
                };
                let guid = e.guid;
                let inv = g.world.inventory.as_ref().map(|i| &i.state);
                let data = inv.and_then(|s| s.items.get(&u)).cloned();
                let ours = inv.is_some_and(|s| {
                    s.holds(p, u)
                        || data
                            .as_ref()
                            .and_then(|d| d.inv)
                            .is_some_and(|owner| s.holds(p, owner))
                });
                let unit_mode = g
                    .events
                    .action
                    .with(&mut g.game, |_, v| v.units.get(u).map(|r| r.mode))
                    .unwrap_or(0) as u8;
                if unit_mode == mode::GROUND {
                    // The inventory model keeps a ground item's position
                    // in its item data (`inventory.md` §2.2); an item
                    // the model has not seen yet has its path's.
                    let (x, y) = data
                        .as_ref()
                        .map(|d| (d.x, d.y))
                        .unwrap_or_else(|| g.events.action.hooks().path_position(u));
                    out.insert(guid, (code, (mode::GROUND, 0, 0, x, y)));
                } else if ours {
                    let d = data.unwrap();
                    out.insert(guid, (code, (d.mode, d.body_loc, d.page, d.x, d.y)));
                }
            }
            out
        })
    }

    /// The client model's items, as [`Self::server_items`].
    fn client_items(&self) -> BTreeMap<u32, ([u8; 4], Place)> {
        let w = self.bridge().world();
        items::items(w)
            .into_iter()
            .map(|i| {
                // A ground item's page and body location mean nothing
                // (compared as 0 on both sides).
                let (body, page) = if i.mode == mode::GROUND {
                    (0, 0)
                } else {
                    (i.body, i.page)
                };
                (
                    i.key.guid,
                    (
                        i.code.unwrap_or([0; 4]),
                        (i.mode, body, page, i32::from(i.x), i32::from(i.y)),
                    ),
                )
            })
            .collect()
    }

    /// The step's checks: no refused intent, no dropped / unowned /
    /// rejected / discarded S→C message since the join, and the client's
    /// items equal the server's.
    fn check(&mut self, what: &str) {
        let refused = std::mem::take(&mut *self.refused.lock().unwrap());
        assert!(refused.is_empty(), "{what}: refused intents {refused:?}");
        let log = self.bridge().log();
        assert_eq!(log.unowned, self.base.0, "{what}: S→C without a handler");
        assert_eq!(log.dropped, self.base.1, "{what}: S→C dropped");
        assert_eq!(
            log.rejected.len(),
            self.base.2,
            "{what}: S→C rejected {:?}",
            &log.rejected[self.base.2..]
        );
        assert_eq!(
            log.discarded.len(),
            self.base.3,
            "{what}: S→C discarded {:?}",
            &log.discarded[self.base.3..]
        );
        let (s, c) = (self.server_items(), self.client_items());
        assert_eq!(c, s, "{what}: client items (left) vs server items (right)");
    }

    /// The GUID of the one item with `code` (server store).
    fn guid(&self, code: [u8; 4]) -> u32 {
        let v: Vec<u32> = self
            .server_items()
            .into_iter()
            .filter(|(_, (c, _))| *c == code)
            .map(|(g, _)| g)
            .collect();
        assert_eq!(v.len(), 1, "one {:?}: {:?}", code, self.server_items());
        v[0]
    }

    fn place_of(&self, guid: u32) -> Place {
        self.server_items()[&guid].1
    }

    /// The player's room origin and position (sub-tiles).
    fn player_at(&self) -> (i32, i32) {
        app_support::with(&self.server, |l| {
            let g = &mut l.host_mut().game;
            let (p, _) = single_player::local_player(g).unwrap();
            g.events.action.hooks().path_position(p)
        })
    }

    /// A monster of `class` at (x, y), by hand.
    fn place_monster(&mut self, class: u32, at: (i32, i32)) -> UnitId {
        app_support::with(&self.server, move |l| {
            let g = &mut l.host_mut().game;
            let (p, _) = single_player::local_player(g).unwrap();
            let room = g.game.lists.unit(p).and_then(|u| u.room()).unwrap();
            let req = AllocRequest {
                ty: UnitType::Monster,
                class,
                room: Some(room),
                add: true,
                fixed_guid: None,
                mode: 1,
                allied: false,
            };
            g.events
                .action
                .with(&mut g.game, |g, v| v.allocate(g, &req, at.0, at.1))
                .unwrap_or_else(|| panic!("monster {class}"))
        })
    }

    /// The player kills `m` (`damage.md` §7.2).
    fn kill(&mut self, m: UnitId) {
        app_support::with(&self.server, move |l| {
            let sim = &mut l.host_mut().game;
            let (p, _) = single_player::local_player(sim).unwrap();
            sim.events.action.combat(&mut sim.game, |w, _| {
                d2_sim::wiring::action::reaction::kill(w, m, p);
            });
        });
    }

    fn interact(&mut self, key: UnitKey) {
        self.app
            .world_mut()
            .resource_mut::<BridgeResource>()
            .0
            .interact(key)
            .unwrap();
    }

    /// Sets a base stat of the player on the server.
    fn set_base(&mut self, stat: u16, value: i32) {
        app_support::with(&self.server, move |l| {
            let g = &mut l.host_mut().game;
            let (p, _) = single_player::local_player(g).unwrap();
            g.events
                .action
                .with(&mut g.game, |_, v| v.set_base(p, stat, value));
        });
    }
}

// Covers: specs/items/treasure.md §3.1; specs/items/inventory-moves.md §7.1, §7.2, §7.3, §7.4, §7.14, §8.1, §8.2
#[test]
fn drops_pick_up_grid_belt_cursor_and_ground() {
    let mut rig = Rig::new();
    let (px, py) = rig.player_at();

    // A monster dies by the player's hand: its treasure class drops the
    // sword, the cap and gold on the ground, and the client sees them.
    let m = rig.place_monster(smoke::MONSTER, (px + 1, py));
    rig.step(5);
    rig.kill(m);
    rig.step(20);
    rig.check("monster drop");
    let ground: Vec<[u8; 4]> = rig
        .server_items()
        .values()
        .filter(|(_, p)| p.0 == mode::GROUND)
        .map(|(c, _)| *c)
        .collect();
    for c in [smoke::SWORD, smoke::CAP, smoke::GOLD] {
        assert!(ground.contains(&c), "{c:?} on the ground: {ground:?}");
    }

    // The chests: operated, one drops its treasure class (§8.1 rule 5:
    // a quarter of the plain chests drop nothing; three are placed).
    let chests: Vec<UnitKey> = rig
        .bridge()
        .world()
        .units
        .iter()
        .filter(|(k, u)| k.unit_type == OBJECT && u.class == single_player::SYNTHETIC_CHEST_CLASS)
        .map(|(k, _)| *k)
        .collect();
    assert_eq!(chests.len(), CHESTS.len(), "the chests in the model");
    for chest in chests {
        let has_axe = |rig: &Rig| rig.server_items().values().any(|(c, _)| *c == smoke::AXE);
        if has_axe(&rig) {
            break;
        }
        rig.interact(chest);
        rig.step(60);
        rig.check("chest");
        assert_ne!(
            rig.bridge().world().units[&chest].mode,
            0,
            "the chest opened"
        );
    }
    for c in [
        smoke::AXE,
        smoke::POTION,
        smoke::IDENTIFY,
        smoke::PORTAL,
        smoke::CHARM,
    ] {
        assert_eq!(rig.place_of(rig.guid(c)).0, mode::GROUND, "{c:?}");
    }

    // Gold: picked up, it leaves the ground and adds to the gold stat.
    let gold = rig.guid(smoke::GOLD);
    rig.act("pick gold", &items::pick(gold, false));
    assert!(
        !rig.server_items().contains_key(&gold),
        "the gold pile is gone"
    );
    rig.step(30);
    let local = rig.bridge().world().local_player.unwrap();
    assert!(
        rig.bridge().world().total(local, 14, 0) > 0,
        "the gold stat rose"
    );

    // Pick-up to the inventory (auto placement) and to the cursor.
    let sword = rig.guid(smoke::SWORD);
    rig.act("pick sword", &items::pick(sword, false));
    assert_eq!(rig.place_of(sword).0, mode::STORED);
    let cap = rig.guid(smoke::CAP);
    rig.act("pick cap to cursor", &items::pick(cap, true));
    assert_eq!(rig.place_of(cap).0, mode::CURSOR);
    // Cursor → grid cell (5, 0) → cursor → grid.
    rig.act("cap to grid", &items::insert(cap, 5, 0, 0));
    assert_eq!(rig.place_of(cap).0, mode::STORED);
    rig.act("cap to cursor", &items::remove(cap));
    assert_eq!(rig.place_of(cap).0, mode::CURSOR);
    // Cursor → ground.
    rig.act("drop cap", &items::drop(cap));
    assert_eq!(rig.place_of(cap).0, mode::GROUND);

    // The potion: to the cursor, then the belt slot 0, back to the cursor
    // (0x24), back to the belt.
    let hp = rig.guid(smoke::POTION);
    rig.act("pick potion to cursor", &items::pick(hp, true));
    rig.act("potion to belt", &items::to_belt(hp, 0));
    assert_eq!(rig.place_of(hp).0, mode::BELT);
    let mut m = vec![0x24];
    m.extend_from_slice(&hp.to_le_bytes());
    rig.act_bytes("potion from belt", &m);
    assert_eq!(rig.place_of(hp).0, mode::CURSOR);
    rig.act("potion to belt again", &items::to_belt(hp, 0));
    assert_eq!(rig.place_of(hp).0, mode::BELT);
}
