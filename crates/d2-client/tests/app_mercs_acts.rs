// Spec: specs/world/npc.md §3, §7.3; specs/world/hirelings.md §3
//! (q-mercs-acts) Mercenary hiring in acts 2, 3 and 5, headless: Greiz,
//! Asheara and Qual-Kehk each sell the mercenary of their synthetic
//! `hireling` row, and the hired unit reaches the client's model.
//! PROVISIONAL (REC-157): the rows' classes, prices and stats.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use d2_client::app::play::{
    add_client_data, add_game, add_walk, predict_link, send_create_game_for,
};
use d2_client::app::server_thread::ThreadLink;
use d2_client::app::single_player::{self, GameData};
use d2_client::bridge::link::{LinkError, Pumped, SendQueue, Sent, ServerLink};
use d2_client::bridge::predict::Speeds;
use d2_client::bridge::world::UnitKey;
use d2_client::bridge::BridgeResource;
use d2_server::seams::Clock;
use d2_sim::missiles::seams::MissileBodies;
use d2_sim::world::npc::class;

mod app_support;
use app_support::SharedLink;

/// The host clock, advanced by the test.
struct StepClock(Arc<AtomicU32>);

impl Clock for StepClock {
    fn now_ms(&mut self) -> u32 {
        self.0.load(Ordering::SeqCst)
    }
}

/// Wraps the shared link and notes the id of every S→C message.
struct Tap {
    inner: SharedLink<ThreadLink<d2_client::app::single_player::Link<StepClock>>>,
    seen: Arc<Mutex<Vec<u8>>>,
    /// The result code (byte 2) of every S→C 0x2A.
    codes: Arc<Mutex<Vec<u8>>>,
}

impl ServerLink for Tap {
    fn protocol_version(&self) -> u32 {
        self.inner.protocol_version()
    }
    fn send(&mut self, queue: SendQueue, msg: &[u8]) -> Result<Sent, LinkError> {
        self.inner.send(queue, msg)
    }
    fn pump(&mut self) -> Result<Pumped, LinkError> {
        self.inner.pump()
    }
    fn receive(&mut self) -> Vec<Vec<u8>> {
        let v = self.inner.receive();
        self.codes
            .lock()
            .unwrap()
            .extend(v.iter().filter(|m| m[0] == 0x2A).map(|m| m[2]));
        self.seen
            .lock()
            .unwrap()
            .extend(v.iter().filter_map(|m| m.first().copied()));
        v
    }
}

/// The server player's level id.
fn server_level(server: &app_support::Server<StepClock>) -> Option<u32> {
    app_support::with(server, |l| {
        let (p, _) = single_player::local_player(&l.host().game)?;
        let g = &mut l.host_mut().game;
        // Qual-Kehk sells only once his quest flag is set (`npc.md` §7.3
        // step 1: quest 36 bit 0); the others ignore it.
        g.world.rest.quests.get_mut(&p).unwrap().flags[0].set(36, 0);
        let room = g.game.lists.unit(p)?.room()?;
        g.events.action.hooks().drlg.level_id(&g.game, room)
    })
}

/// One mercenary seller: the town to reach, the seller's class and place
/// (stand-beside point), the first name id of its hire rows and the
/// mercenary class it sells.
struct Seller {
    /// The act change target (`None`: the starting town with `npcs`).
    level: Option<u32>,
    npcs: Vec<(u16, i32)>,
    class: u16,
    stand: (i32, i32),
    name: u16,
    merc: u32,
}

fn sellers() -> Vec<Seller> {
    let act3 = d2_client::app::town_npcs::ACT3;
    let act5 = d2_client::app::town_npcs::ACT5;
    let at = |l: &[(u16, i32)], c: u16| l.iter().find(|n| n.0 == c).unwrap().1;
    let greiz = single_player::ACT2_NPCS
        .iter()
        .position(|&c| c == class::GREIZ)
        .unwrap() as i32;
    vec![
        Seller {
            level: Some(single_player::ACT2_TOWN),
            npcs: Vec::new(),
            class: class::GREIZ,
            stand: (
                single_player::ACT2_NPC_X0 + 4 * greiz,
                single_player::ACT2_NPC_Y + 3,
            ),
            name: 2000,
            merc: 271,
        },
        Seller {
            level: None,
            npcs: act3.to_vec(),
            class: class::ASHEARA,
            // The town room of act 1 starts at tile 16.
            stand: (
                16 * 5 + at(&act3, class::ASHEARA),
                single_player::UNIT_Y + 3,
            ),
            name: 3000,
            merc: 357,
        },
        Seller {
            level: Some(single_player::ACT5_TOWN),
            npcs: Vec::new(),
            class: class::QUAL_KEHK,
            stand: (at(&act5, class::QUAL_KEHK), single_player::UNIT_Y + 3),
            name: 3100,
            merc: 560,
        },
    ]
}

// Covers: specs/world/npc.md §7.3; specs/world/hirelings.md §3.2
#[test]
fn greiz_asheara_and_qual_kehk_each_hire_a_mercenary_that_reaches_the_model() {
    for s in sellers() {
        session(&s);
    }
}

fn session(s: &Seller) {
    let data = GameData::Synthetic;
    let ms = Arc::new(AtomicU32::new(1000));
    let character = single_player::new_character("sorceress", "Test").unwrap();
    let (link, _) = single_player::start_with_town(
        data.clone(),
        single_player::DEFAULT_SEED,
        character.clone(),
        StepClock(ms.clone()),
        Vec::new(),
        None,
        if s.npcs.is_empty() {
            d2_client::app::town_npcs::ACT1.to_vec()
        } else {
            s.npcs.clone()
        },
    )
    .unwrap();
    let server = Arc::new(Mutex::new(link));
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default()))
        .init_asset::<Image>()
        .init_resource::<ButtonInput<MouseButton>>();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let codes = Arc::new(Mutex::new(Vec::new()));
    let (link, tap) = predict_link(Box::new(Tap {
        inner: SharedLink(server.clone()),
        seen: seen.clone(),
        codes: codes.clone(),
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
    let mut steps = 0;
    let mut step = |app: &mut App, n: usize| {
        for _ in 0..n {
            ms.fetch_add(40, Ordering::SeqCst);
            app.update();
            steps += 1;
            assert!(steps < 6000, "the test finishes");
        }
    };
    while app_support::local_player(&server).is_none() {
        step(&mut app, 1);
    }
    step(&mut app, 30);
    if let Some(level) = s.level {
        app_support::with(&server, move |l| {
            let (p, _) = single_player::local_player(&l.host().game).unwrap();
            l.host_mut()
                .game
                .events
                .action
                .hooks()
                .act_changes
                .push((p, level, 0));
        });
        while server_level(&server) != Some(level) {
            step(&mut app, 1);
        }
        step(&mut app, 30);
    }
    let units = |app: &App| -> Vec<(UnitKey, u32)> {
        app.world()
            .resource::<BridgeResource>()
            .0
            .world()
            .units
            .iter()
            .map(|(k, u)| (*k, u.class))
            .collect()
    };
    let before = units(&app);
    assert!(
        !before.iter().any(|(_, c)| *c == s.merc),
        "no mercenary yet: {before:?}"
    );
    let Some(&(key, _)) = before
        .iter()
        .find(|(k, c)| k.unit_type == 1 && *c == u32::from(s.class))
    else {
        panic!("seller {} is in the model: {before:?}", s.class);
    };
    // Stand beside the seller.
    let (x, y) = s.stand;
    app_support::with(&server, move |l| {
        let (p, _) = single_player::local_player(&l.host().game).unwrap();
        let g = &mut l.host_mut().game;
        let room = g.game.lists.unit(p).and_then(|u| u.room());
        g.events
            .action
            .with(&mut g.game, |g, v| v.path_teleport(g, p, room, x, y));
    });
    step(&mut app, 3);
    app.world_mut()
        .resource_mut::<BridgeResource>()
        .0
        .interact(key)
        .unwrap();
    let talks =
        |seen: &Arc<Mutex<Vec<u8>>>| seen.lock().unwrap().iter().filter(|&&i| i == 0x28).count();
    for _ in 0..100 {
        step(&mut app, 1);
        if talks(&seen) > 0 {
            break;
        }
    }
    assert!(talks(&seen) > 0, "seller {} talks", s.class);
    let rejected_before = app
        .world()
        .resource::<BridgeResource>()
        .0
        .log()
        .rejected
        .len();
    // The hire (C→S 0x36: the seller's GUID and the first name id).
    let mut msg = vec![0x36];
    msg.extend(key.guid.to_le_bytes());
    msg.extend(u32::from(s.name).to_le_bytes());
    app.world_mut()
        .resource_mut::<BridgeResource>()
        .0
        .send_bytes(&msg)
        .unwrap();
    for _ in 0..100 {
        step(&mut app, 1);
        if units(&app).iter().any(|(_, c)| *c == s.merc) {
            break;
        }
    }
    let after = units(&app);
    assert!(
        after.iter().any(|(k, c)| k.unit_type == 1 && *c == s.merc),
        "the hired class {} is in the model: {after:?}",
        s.merc
    );
    // The mercenary dies (the host's pet death queue), then the seller
    // resurrects it (C→S 0x62, `npc.md` §7.4): S→C 0x2A code 5 (the
    // mercenary's GUID) after the cost is paid.
    let merc_guid = after
        .iter()
        .find(|(k, c)| k.unit_type == 1 && *c == s.merc)
        .map(|(k, _)| k.guid)
        .unwrap();
    app_support::with(&server, move |l| {
        let g = &mut l.host_mut().game;
        let merc = g
            .game
            .lists
            .find_unit(d2_sim::units::UnitType::Monster, merc_guid)
            .expect("the mercenary is on the server");
        g.events
            .action
            .hooks()
            .pet_deaths
            .as_mut()
            .unwrap()
            .push(merc);
    });
    step(&mut app, 5);
    // The synthetic game has a stat table now (q-a4-quest-items), so the
    // resurrect costs what its level asks: the player carries the gold
    // (stat 14).
    app_support::with(&server, |l| {
        let g = &mut l.host_mut().game;
        let (p, _) = single_player::local_player(g).unwrap();
        g.events
            .action
            .with(&mut g.game, |_, v| v.set_base(p, 14, 1_000_000));
    });
    let before = codes.lock().unwrap().len();
    let mut msg = vec![0x62];
    msg.extend(key.guid.to_le_bytes());
    app.world_mut()
        .resource_mut::<BridgeResource>()
        .0
        .send_bytes(&msg)
        .unwrap();
    step(&mut app, 20);
    let tail: Vec<u8> = codes.lock().unwrap()[before..].to_vec();
    assert_eq!(tail, [5], "seller {}: the resurrect's result code", s.class);
    // Act III's far NPCs stand outside the synthetic town room and are
    // rejected on creation (0x13C, preview layout, REC-137); the hire adds
    // no rejection.
    let b = &app.world().resource::<BridgeResource>().0;
    assert_eq!(
        b.log().rejected.len(),
        rejected_before,
        "seller {}: {:?}",
        s.class,
        b.log().rejected
    );
}
