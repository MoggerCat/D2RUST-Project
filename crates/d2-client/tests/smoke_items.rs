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
/// The stash's sub-tile: beside the player's start, in reach.
const STASH: (i32, i32) = (single_player::WAYPOINT_X + 1, single_player::UNIT_Y + 5);

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
        // The item tables the play app installs from the user's files
        // (`app/play.rs`, `TableDecoder`): here the synthetic ones.
        app.world_mut()
            .resource_mut::<BridgeResource>()
            .0
            .set_item_tables(Arc::new(d2_client::app::items::TableDecoder(Arc::new(
                d2_client::app::synthetic_items::item_tables(),
            ))));
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

    /// The smoke monster dies beside the player: the sword, the cap and
    /// gold on the ground, announced.
    fn monster_drop(&mut self) {
        let (px, py) = self.player_at();
        let m = self.place_monster(smoke::MONSTER, (px + 1, py));
        self.step(5);
        self.kill(m);
        self.step(20);
        self.check("monster drop");
    }

    /// The item's own value of `stat` on the server (its stat list total).
    fn server_item_stat(&self, guid: u32, stat: u16) -> i32 {
        app_support::with(&self.server, move |l| {
            let g = &mut l.host_mut().game;
            let u = g.game.lists.find_unit(UnitType::Item, guid).unwrap();
            g.events
                .action
                .with(&mut g.game, |_, v| v.stats.unit_total(u, stat, 0))
        })
    }

    /// The local player's total of `stat` on the server.
    fn server_unit_stat(&self, stat: u16) -> i32 {
        app_support::with(&self.server, move |l| {
            let g = &mut l.host_mut().game;
            let (p, _) = single_player::local_player(g).unwrap();
            g.events
                .action
                .with(&mut g.game, |_, v| v.stats.unit_total(p, stat, 0))
        })
    }

    /// The local player's (base, total) of `stat` in the client model.
    fn client_stat(&self, stat: u16) -> (i32, i32) {
        let w = self.bridge().world();
        let me = w.local_player.unwrap();
        (w.base(me, stat, 0), w.total(me, stat, 0))
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
                // The player's items and the fillers socketed into them.
                let ours = inv.is_some_and(|s| {
                    s.holds(p, u)
                        || s.items_of(p)
                            .into_iter()
                            .any(|it| s.fillers(it).contains(&u))
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
                    let body = if d.mode == mode::BODY { d.body_loc } else { 0 };
                    out.insert(guid, (code, (d.mode, body, d.page, d.x, d.y)));
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
                // (compared as 0 on both sides); the body location only
                // in mode 1 (the stream carries item data +0x44 as is,
                // `bitstream.md` §4.1 rule 3; nothing clears it when the
                // item leaves the body, `inventory-moves.md` §7.7).
                let (body, page) = if i.mode == mode::GROUND {
                    (0, 0)
                } else if i.mode != mode::BODY {
                    (0, i.page)
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
    open_chests(&mut rig);
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

    // Auto pick-up: the identified sword goes to the empty right hand
    // (`inventory-moves.md` §8.1 step 5, `inventory.md` §4.7, §4.9); the
    // axe (strength 32 > 10) fails §4.2 and goes to the grid (step 7).
    let sword = rig.guid(smoke::SWORD);
    rig.act("pick sword", &items::pick(sword, false));
    assert_eq!(rig.place_of(sword).0, mode::BODY);
    assert_eq!(rig.place_of(sword).1, 4);
    let axe = rig.guid(smoke::AXE);
    rig.act("pick axe", &items::pick(axe, false));
    assert_eq!(rig.place_of(axe).0, mode::STORED);
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

// Covers: specs/items/inventory-moves.md §7.1 r2
#[test]
fn a_far_pick_up_runs_to_the_item_and_picks_it() {
    let mut rig = Rig::new();
    // The walk-verify S→C 0x96 a run brings needs the render seam's
    // visibility predicate (`model.md` open question 7, q-fix-visibility
    // wires it in the app): given here as an input, every sprite seen.
    rig.app
        .world_mut()
        .resource_mut::<BridgeResource>()
        .0
        .set_visibility(Some(|_, _, _| true));
    let (px, py) = rig.player_at();
    // A monster dies 8 sub-tiles away: its drop lies out of the
    // pick-up reach (distance ≥ 5, §7.1 step 2).
    let m = rig.place_monster(smoke::MONSTER, (px + 8, py));
    rig.step(5);
    rig.kill(m);
    rig.step(20);
    rig.check("far drop");
    let sword = rig.guid(smoke::SWORD);
    let (_, _, _, sx, sy) = rig.place_of(sword);
    assert!((sx - px).abs().max((sy - py).abs()) >= 5, "out of reach");
    // One 0x16: the server runs the player there and picks it up on
    // arrival (to the inventory, cursor flag 0).
    rig.send(&items::pick(sword, false));
    for _ in 0..200 {
        rig.step(1);
        if rig.place_of(sword).0 == mode::BODY {
            break;
        }
    }
    rig.step(10);
    rig.check("far pick-up");
    assert_eq!(rig.place_of(sword).0, mode::BODY, "picked on arrival");
    let (ax, ay) = rig.player_at();
    assert!(
        (ax - sx).abs().max((ay - sy).abs()) < 5,
        "the player ran to the item: ({ax}, {ay}) for ({sx}, {sy})"
    );
}

// Covers: specs/items/inventory-moves.md §7.5, §7.7; specs/items/inventory.md §4.2
#[test]
fn equip_unequip_requirements_and_weapon_swap() {
    let mut rig = Rig::new();
    rig.monster_drop();
    let (cap, sword) = (rig.guid(smoke::CAP), rig.guid(smoke::SWORD));
    // The cap to the head (body location 1) from the cursor.
    rig.act("pick cap", &items::pick(cap, true));
    let defense = rig.client_stat(31);
    rig.act("equip cap", &items::equip(cap, 1));
    assert_eq!(rig.place_of(cap).0, mode::BODY);
    // The character panel's defense: base unchanged, total + the cap's
    // (`client/stat-lists.md` §2: the equipped item's list adds).
    let cap_def = rig.server_item_stat(cap, 31);
    assert!(cap_def >= 3, "the cap's defense {cap_def}");
    assert_eq!(rig.client_stat(31), (defense.0, defense.1 + cap_def));
    // Unequip to the cursor (0x1C), back on.
    rig.act_bytes("unequip cap", &[0x1C, 1, 0]);
    assert_eq!(rig.place_of(cap).0, mode::CURSOR);
    rig.act("equip cap again", &items::equip(cap, 1));
    assert_eq!(rig.place_of(cap).0, mode::BODY);
    // The sword to the right hand (4).
    rig.act("pick sword", &items::pick(sword, true));
    rig.act("equip sword", &items::equip(sword, 4));
    assert_eq!(rig.place_of(sword).1, 4);
    // W: the weapon switch (C→S 0x60): the sword goes to the switch
    // slot (11), the empty second set comes to the hands.
    rig.act_bytes("weapon swap", &[0x60]);
    assert_eq!(rig.place_of(sword).1, 11);
    // The axe needs strength 32: at 10, §4.6 step 2 (§4.3 with §4.2)
    // fails, result 0, it stays on the cursor.
    open_chests(&mut rig);
    let axe = rig.guid(smoke::AXE);
    rig.act("pick axe", &items::pick(axe, true));
    rig.act("equip axe at strength 10", &items::equip(axe, 4));
    assert_eq!(rig.place_of(axe).0, mode::CURSOR);
    rig.set_base(0, 40);
    rig.step(5);
    rig.act("equip axe at strength 40", &items::equip(axe, 4));
    assert_eq!((rig.place_of(axe).0, rig.place_of(axe).1), (mode::BODY, 4));
    // W again: the sets trade places.
    rig.act_bytes("weapon swap back", &[0x60]);
    let hands = |rig: &Rig| [rig.place_of(sword).1, rig.place_of(axe).1];
    assert_eq!(hands(&rig), [4, 11]);
}

/// The chests in the client model.
fn chests(rig: &Rig) -> Vec<UnitKey> {
    rig.bridge()
        .world()
        .units
        .iter()
        .filter(|(k, u)| k.unit_type == OBJECT && u.class == single_player::SYNTHETIC_CHEST_CLASS)
        .map(|(k, _)| *k)
        .collect()
}

/// Opens the chests until one dropped its class (§8.1 rule 5: a quarter
/// of the plain chests drop nothing; three are placed).
fn open_chests(rig: &mut Rig) {
    let chests = chests(rig);
    assert_eq!(chests.len(), CHESTS.len(), "the chests in the model");
    for chest in chests {
        if rig.server_items().values().any(|(c, _)| *c == smoke::AXE) {
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
}

impl Rig {
    /// Item flags of `guid` on the server (item store).
    fn server_flags(&self, guid: u32) -> u32 {
        app_support::with(&self.server, move |l| {
            let g = &mut l.host_mut().game;
            let u = g.game.lists.find_unit(UnitType::Item, guid).unwrap();
            g.events.action.hooks().items.get(u).map_or(0, |i| i.flags)
        })
    }

    /// The client's header flags of `guid` (its last record).
    fn client_flags(&self, guid: u32) -> u32 {
        items::items(self.bridge().world())
            .into_iter()
            .find(|i| i.key.guid == guid)
            .map_or(0, |i| i.flags)
    }

    fn exists(&self, guid: u32) -> bool {
        self.server_items().contains_key(&guid)
    }
}

/// Item flag 0x10 (identified).
const IDENTIFIED: u32 = 0x10;

// Covers: specs/items/inventory-moves.md §7.11, §7.17, §7.18, §7.19
#[test]
fn sockets_charm_identify_potion_and_scrolls() {
    let mut rig = Rig::new();
    // The sword drops with its two sockets (`generation.md` §7: the
    // socket roll is random; the test sets the outcome before the drop
    // is announced: item flag 0x800, stat 194 = 2).
    let (px, py) = rig.player_at();
    let m = rig.place_monster(smoke::MONSTER, (px + 1, py));
    rig.step(5);
    rig.kill(m);
    app_support::with(&rig.server, |l| {
        let g = &mut l.host_mut().game;
        let row = d2_client::app::synthetic_items::item_tables()
            .items
            .iter()
            .position(|r| r.code == smoke::SWORD);
        let sword = g
            .game
            .lists
            .units_of_type(UnitType::Item)
            .into_iter()
            .find(|&u| g.events.action.hooks().items.get(u).map(|i| i.record) == row)
            .expect("the sword dropped");
        if let Some(i) = g.events.action.hooks().items.get_mut(sword) {
            i.flags |= 0x800;
        }
        g.events
            .action
            .with(&mut g.game, |_, v| v.set_base(sword, 194, 2));
    });
    rig.step(20);
    rig.check("monster drop");
    open_chests(&mut rig);
    // Socketing: the gem from the cursor into the worn sword.
    let (sword, gem) = (rig.guid(smoke::SWORD), rig.guid(*b"gsv "));
    rig.act("pick sword", &items::pick(sword, false));
    assert_eq!(rig.place_of(sword).0, mode::BODY);
    rig.act("pick gem", &items::pick(gem, true));
    let mut m = vec![0x28];
    m.extend_from_slice(&gem.to_le_bytes());
    m.extend_from_slice(&sword.to_le_bytes());
    rig.act_bytes("socket gem", &m);
    assert_eq!(rig.place_of(gem).0, mode::SOCKETED, "the gem in the sword");
    // The charm, carried in the inventory (page 0).
    let charm = rig.guid(smoke::CHARM);
    let strength = rig.client_stat(0);
    rig.act("pick charm", &items::pick(charm, false));
    assert_eq!(rig.place_of(charm).0, mode::STORED);
    // Unidentified it counts for nothing (`inventory.md` §4.2 identified;
    // the stream carries no list, `bitstream.md` §4.6).
    rig.step(10);
    assert_eq!(rig.client_stat(0), strength, "an unidentified charm");
    assert_eq!(rig.server_flags(charm) & IDENTIFIED, 0, "a magic charm");
    assert_eq!(rig.client_flags(charm) & IDENTIFIED, 0);
    // Identify with the scroll: use it (0x20), then on the charm (0x27).
    let isc = rig.guid(smoke::IDENTIFY);
    rig.act("pick identify scroll", &items::pick(isc, false));
    let (px, py) = rig.player_at();
    rig.act(
        "use identify scroll",
        &items::use_grid(isc, px as u32, py as u32),
    );
    let mut m = vec![0x27];
    m.extend_from_slice(&charm.to_le_bytes());
    m.extend_from_slice(&isc.to_le_bytes());
    rig.act_bytes("identify the charm", &m);
    assert_ne!(rig.server_flags(charm) & IDENTIFIED, 0, "identified");
    assert_ne!(rig.client_flags(charm) & IDENTIFIED, 0, "the client knows");
    assert!(!rig.exists(isc), "the scroll was used up");
    // Identified and carried on page 0, its list adds to the total
    // (`inventory.md` §5.7; `client/stat-lists.md` §2), the base the same.
    rig.step(10);
    assert_eq!(
        rig.client_stat(0),
        (strength.0, strength.1 + smoke::CHARM_STR),
        "the charm's strength"
    );
    assert_eq!(rig.server_unit_stat(0), strength.1 + smoke::CHARM_STR);
    // The potion: to the belt, drink it at half life.
    let hp = rig.guid(smoke::POTION);
    rig.act("pick potion", &items::pick(hp, false));
    assert_eq!(rig.place_of(hp).0, mode::BELT);
    rig.set_base(6, 20 << 8);
    rig.step(10);
    let life = rig.client_stat(6).1;
    rig.act("drink potion", &items::use_belt(hp));
    assert!(!rig.exists(hp), "the potion was drunk");
    rig.step(60);
    assert!(rig.client_stat(6).1 > life, "life rose from {life}");
    // The town portal scroll, used from the grid (0x20).
    let tsc = rig.guid(smoke::PORTAL);
    rig.act("pick portal scroll", &items::pick(tsc, false));
    assert_eq!(rig.place_of(tsc).0, mode::STORED);
    let (px, py) = rig.player_at();
    rig.act(
        "read portal scroll",
        &items::use_grid(tsc, px as u32, py as u32),
    );
    rig.step(20);
    rig.check("after the portal");
    // Read in town with no field cast before: the scroll is used and no
    // portal opens (`wiring::action::town_portal`, REC-243).
    assert!(!rig.exists(tsc), "the scroll was read");
}

// Covers: specs/items/inventory-moves.md §7.3, §7.4; specs/world/npc.md §6
#[test]
fn stash_cube_and_cain_identify() {
    let mut rig = Rig::new();
    rig.monster_drop();
    open_chests(&mut rig);
    let cap = rig.guid(smoke::CAP);
    // The stash (page 4, a town level), opened: cursor → stash → cursor.
    let stash = rig
        .bridge()
        .world()
        .units
        .iter()
        .find(|(k, u)| k.unit_type == OBJECT && u.class == single_player::STASH_CLASS)
        .map(|(k, _)| *k)
        .expect("the stash in the model");
    // (A busy player picks nothing up, §8.2: the cap first.)
    rig.act("pick cap", &items::pick(cap, true));
    rig.interact(stash);
    rig.step(20);
    rig.check("open the stash");
    rig.act("cap to the stash", &items::insert(cap, 0, 0, 4));
    assert_eq!(
        (rig.place_of(cap).0, rig.place_of(cap).2),
        (mode::STORED, 4)
    );
    rig.act("cap out of the stash", &items::remove(cap));
    assert_eq!(rig.place_of(cap).0, mode::CURSOR);
    rig.act("cap to the grid", &items::insert(cap, 2, 0, 0));
    // The interaction ends (C→S 0x4F button 0x12, the stash closed).
    rig.act_bytes("close the stash", &[0x4F, 0x12, 0, 0, 0, 0, 0]);
    // The cube page (3) needs no open cube to put in (§7.3 step 3); to
    // take out, the cube is opened by its use (0x20, `cube.md` §2).
    let cube = rig.guid(smoke::CUBE);
    rig.act("pick cube", &items::pick(cube, false));
    assert_eq!(rig.place_of(cube).0, mode::STORED);
    rig.act("cap to cursor", &items::remove(cap));
    rig.act("cap to the cube", &items::insert(cap, 0, 0, 3));
    assert_eq!(
        (rig.place_of(cap).0, rig.place_of(cap).2),
        (mode::STORED, 3)
    );
    let (px, py) = rig.player_at();
    rig.act(
        "open the cube",
        &items::use_grid(cube, px as u32, py as u32),
    );
    rig.act("cap out of the cube", &items::remove(cap));
    rig.act_bytes("close the cube", &[0x4F, 0x17, 0, 0, 0, 0, 0]);
    rig.act("cap to the grid again", &items::insert(cap, 2, 0, 0));
    assert_eq!(
        (rig.place_of(cap).0, rig.place_of(cap).2),
        (mode::STORED, 0)
    );

    // Cain identifies the unidentified charm for 100 gold (`npc.md` §6).
    let charm = rig.guid(smoke::CHARM);
    rig.act("pick charm", &items::pick(charm, false));
    assert_eq!(rig.server_flags(charm) & IDENTIFIED, 0);
    rig.set_base(14, 1000);
    rig.step(10);
    let (px, py) = rig.player_at();
    let cain = rig.place_monster(
        u32::from(d2_sim::world::quests::act4::q3::CAIN4),
        (px + 2, py),
    );
    rig.step(10);
    let cain_guid = app_support::with(&rig.server, move |l| {
        l.host().game.game.lists.unit(cain).unwrap().guid
    });
    let key = UnitKey {
        unit_type: d2_client::bridge::world::MONSTER,
        guid: cain_guid,
    };
    rig.interact(key);
    rig.step(20);
    rig.check("talk to Cain");
    let mut m = vec![0x34];
    m.extend_from_slice(&cain_guid.to_le_bytes());
    rig.act_bytes("Cain identifies", &m);
    assert_ne!(rig.server_flags(charm) & IDENTIFIED, 0, "identified");
    assert_ne!(rig.client_flags(charm) & IDENTIFIED, 0, "the client knows");
    rig.step(30);
    assert_eq!(rig.client_stat(14).1, 900, "100 gold for one item");
}
