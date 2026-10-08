// Spec: specs/client/bridge.md (§2, §4, §6, §8), specs/sim/intents-events.md (§2.2, §8); preview fills: docs/PLAN.md decisions D1–D3, docs/handoff/q-play-smoke.md
//! The end-to-end play smoke run (task `q-play-smoke`): one scripted run
//! through the real play path, headless (the bridge, the in-process
//! server and the sim, the play app's systems and its original UI, no
//! window), over the synthetic single-player game. Every file is a
//! synthetic fixture in a memory source, never a game file.
//!
//! After every step the run asserts that the server refused no intent
//! (every drained C→S message dispatched with result 0, `seams.rs`
//! `ResultCode::Done`), that the client model dropped, rejected,
//! discarded and left unhandled no S→C message (`bridge.md` §6), and no
//! system panicked (Bevy's error handler panics on a bridge error).

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use d2_client::app::palette::{add_act_palettes, ActPalettes};
use d2_client::app::play::{
    add_client_data, add_game, add_preview, add_walk, predict_link, send_create_game_for,
};
use d2_client::app::server_thread::ThreadLink;
use d2_client::app::single_player::{self, GameData};
use d2_client::app::ui::{add_original_ui_with, UiParts};
use d2_client::assets::path::MemorySource;
use d2_client::bridge::link::{LinkError, Pumped, SendQueue, Sent, ServerLink};
use d2_client::bridge::local::{LocalLink, PendingSession};
use d2_client::bridge::predict::Speeds;
use d2_client::bridge::world::UnitKey;
use d2_client::bridge::BridgeResource;
use d2_client::rules::unit_composite::code;
use d2_client::ui::layout::{OptionKind, Screen};
use d2_client::ui::original::{FontMeasure, OriginalUi, UiConfig, CHARACTER_FONTS};
use d2_client::ui::{font_info, Point, PointerButton, UiEvent};
use d2_client::world_view::tile_assets::TileAssets;
use d2_client::world_view::unit_assets::UnitLooks;
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

/// A DT1 of one tile with one 32 × 32 RLE block (`formats/dt1.md`), the
/// fixture of `app_play_preview.rs`.
fn dt1_bytes() -> Vec<u8> {
    let encoded = [0x00u8, 0x02, 0x0A, 0x0B];
    let mut d = Vec::new();
    d.extend_from_slice(&7u32.to_le_bytes());
    d.extend_from_slice(&6u32.to_le_bytes());
    d.extend_from_slice(&[0; 260]);
    d.extend_from_slice(&1u32.to_le_bytes());
    d.extend_from_slice(&276u32.to_le_bytes());
    let mut tile = vec![0u8; 96];
    tile[0x48..0x4C].copy_from_slice(&372u32.to_le_bytes());
    tile[0x50..0x54].copy_from_slice(&1u32.to_le_bytes());
    d.extend_from_slice(&tile);
    for v in [0u16, 0, 0] {
        d.extend_from_slice(&v.to_le_bytes());
    }
    d.extend_from_slice(&[0, 0]);
    d.extend_from_slice(&0x1001u16.to_le_bytes());
    d.extend_from_slice(&(encoded.len() as u32).to_le_bytes());
    d.extend_from_slice(&0u16.to_le_bytes());
    d.extend_from_slice(&20u32.to_le_bytes());
    d.extend_from_slice(&encoded);
    d
}

/// A DC6 of one direction with `frames` frames of 2 × 2 literal pixels
/// (`formats/dc6.md`).
fn dc6(frames: u32) -> Vec<u8> {
    let rows = [2u8, 1, 2, 0x80, 2, 3, 4, 0x80];
    let mut d = Vec::new();
    for v in [6i32, 1, 0] {
        d.extend_from_slice(&v.to_le_bytes());
    }
    d.extend_from_slice(&[0xEE; 4]);
    d.extend_from_slice(&1u32.to_le_bytes());
    d.extend_from_slice(&frames.to_le_bytes());
    let mut at = d.len() + 4 * frames as usize;
    let mut body = Vec::new();
    for _ in 0..frames {
        d.extend_from_slice(&(at as u32).to_le_bytes());
        for v in [0u32, 2, 2, 0, 0, 0, 0, rows.len() as u32] {
            body.extend_from_slice(&v.to_le_bytes());
        }
        body.extend_from_slice(&rows);
        body.extend_from_slice(&[0xEE; 3]);
        at += 32 + rows.len() + 3;
    }
    d.extend(body);
    d
}

/// COF bytes (`formats/cof.md`): one direction, one frame, one layer
/// (component 1, weapon class `hth`), animation rate 256.
fn cof_bytes() -> Vec<u8> {
    let mut v = vec![1, 1, 1, 20, 0, 0, 0, 0];
    for x in [-10i32, 10, -20, 0] {
        v.extend_from_slice(&x.to_le_bytes());
    }
    v.extend_from_slice(&256u32.to_le_bytes());
    v.extend_from_slice(&[1, 0, 1, 0, 0]);
    v.extend_from_slice(b"hth\0");
    v.push(0);
    v.push(1);
    v
}

/// A `.tbl` (`formats/font-tbl.md`): 256 records of width 6.
fn tbl() -> Vec<u8> {
    let mut d = b"Woo!".to_vec();
    d.extend_from_slice(&1u16.to_le_bytes());
    d.extend_from_slice(&0u16.to_le_bytes());
    d.extend_from_slice(&256u16.to_le_bytes());
    d.extend_from_slice(&[10, 0]);
    for i in 0..256u16 {
        d.extend_from_slice(&i.to_le_bytes());
        d.extend_from_slice(&[0, 6, 10, 0, 0, 0]);
        d.extend_from_slice(&i.to_le_bytes());
        d.extend_from_slice(&[0; 4]);
    }
    d
}

/// A `pal.pl2` of zeros with its 13 text colours (`formats/palette.md`).
fn pl2() -> Vec<u8> {
    vec![0; 1024 + 1714 * 256 + 13 * (3 + 256)]
}

/// Invented unit tokens: every player class is `OY`, mode 5 `TN`,
/// component 1 `TR` (`app_play_e2e.rs`).
fn looks() -> UnitLooks {
    UnitLooks {
        player_tokens: vec![code(b"OY"); 7],
        player_modes: [b"DT", b"NU", b"WL", b"RN", b"GH", b"TN", b"TW"]
            .iter()
            .map(|m| code(*m))
            .collect(),
        components: vec![code(b"HD"), code(b"TR")],
        ..Default::default()
    }
}

/// Every file the play preview reads here.
fn files() -> MemorySource {
    let mut s = MemorySource::default();
    s.insert(r"DATA\GLOBAL\TILES\floor.dt1", dt1_bytes());
    s.insert(r"data\global\chars\OY\cof\OYTNhth.cof", cof_bytes());
    s.insert(r"data\global\chars\OY\TR\OYTRlitTNhth.dc6", dc6(1));
    let config = UiConfig {
        screen: Screen::R800,
        expansion_installed: false,
    };
    let ui = OriginalUi::new(config, None).unwrap();
    for name in ui.files().names() {
        s.insert(&format!("data\\global\\ui\\{name}.dc6"), dc6(64));
    }
    for id in 0..14 {
        let Some(f) = font_info(id) else { continue };
        s.insert(f.tbl_path, tbl());
        s.insert(f.dc6_path, dc6(256));
    }
    s
}

/// The run: the app, its server thread, the host clock and the wire.
struct Run {
    app: App,
    server: Server<StepClock>,
    ms: Arc<AtomicU32>,
    wire: Arc<Mutex<Wire>>,
    frames: usize,
    character: single_player::Character,
}

/// A character's state for the save round trip: (stat, value), (skill,
/// base level), (item code, mode, body location).
type Snapshot = (Vec<(u16, i32)>, Vec<(u16, i32)>, Vec<([u8; 4], u8, u8)>);

impl Run {
    /// The play app over the synthetic game, as `d2-client play --new`
    /// wires it, with a new sorceress; returns once the join ran.
    fn start() -> Self {
        Self::start_with(|_| {})
    }

    /// [`Self::start`] with `install` run on the built game before the
    /// join (fixture tables the synthetic game lacks).
    fn start_with(install: fn(&mut single_player::Sim)) -> Self {
        Self::start_on(GameData::Synthetic, install)
    }

    /// The play app on `data` (synthetic, or an install loaded as
    /// `d2-client play` loads the user's files), `install` run on the
    /// built game before the join.
    fn start_on(data: GameData, install: fn(&mut single_player::Sim)) -> Self {
        let character = single_player::new_character("sorceress", "Smoke").unwrap();
        Self::start_as(data, character, install)
    }

    /// [`Self::start_on`] with `character` (a new one, or a save).
    fn start_as(
        data: GameData,
        character: single_player::Character,
        install: fn(&mut single_player::Sim),
    ) -> Self {
        let ms = Arc::new(AtomicU32::new(1000));
        let clock = StepClock(ms.clone());
        let built = character.clone();
        let game_data = data.clone();
        let link = ThreadLink::spawn(move || {
            let mut g = single_player::build_with(&game_data, single_player::DEFAULT_SEED, built)?;
            install(&mut g.sim);
            Ok::<_, single_player::BuildError>(LocalLink::new(Host::new(
                g.sim,
                ProtoSizes,
                PendingSession::default(),
                clock,
            )))
        })
        .unwrap();
        let server = Arc::new(Mutex::new(link));
        let wire = Arc::new(Mutex::new(Wire::default()));
        let source = Arc::new(files());
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Image>()
            .init_resource::<ButtonInput<MouseButton>>();
        let probe = Probe {
            server: server.clone(),
            wire: wire.clone(),
        };
        let (link, tap) = predict_link(Box::new(probe));
        add_game(&mut app, link, false).unwrap();
        send_create_game_for(&mut app, &character).unwrap();
        let levels = single_player::client_level_rows(&data);
        add_client_data(
            &mut app,
            single_player::client_drlg_source(&data),
            levels.clone(),
        );
        add_preview(
            &mut app,
            levels,
            TileAssets::new(Some(source.clone()), None),
        );
        let speeds = match &data {
            GameData::Synthetic => {
                app_support::synthetic_skill_rows(&mut app);
                app.world_mut()
                    .resource_mut::<BridgeResource>()
                    .0
                    .set_unit_rows(single_player::synthetic_unit_rows());
                Speeds { walk: 6, run: 9 }
            }
            GameData::Live(d) => {
                // The client tables `play::run` gives the bridge on the
                // user's files.
                let a = d.archives.as_ref();
                let mut b = app.world_mut().resource_mut::<BridgeResource>();
                b.0.set_skill_rows(single_player::client_skill_rows(a).unwrap());
                b.0.set_class_skills(single_player::client_class_skills(a).unwrap());
                b.0.set_skill_tables(Arc::new(single_player::client_skill_tables(a).unwrap()));
                b.0.set_unit_rows(single_player::client_unit_rows(a).unwrap());
                b.0.set_item_tables(Arc::new(d2_client::app::items::TableDecoder(Arc::new(
                    d.tables.item_tables().unwrap(),
                ))));
                b.0.set_object_rows(single_player::client_object_rows(&data));
                single_player::walk_speeds(&data, &character)
                    .unwrap()
                    .expect("walk speeds from charstats")
            }
        };
        add_act_palettes(
            &mut app,
            ActPalettes {
                pl2: std::array::from_fn(|_| pl2()),
                shown: None,
            },
        );
        let fonts = FontMeasure::load(source.as_ref(), &CHARACTER_FONTS).unwrap();
        add_original_ui_with(
            &mut app,
            UiParts {
                source: source.clone(),
                inv_areas: None,
                expansion_installed: false,
                fonts: Some(fonts),
                resist_penalties: Some(vec![0, 20, 50]),
            },
            looks(),
        )
        .unwrap();
        d2_client::app::ui::set_waypoint_map(&mut app, single_player::client_waypoint_map(&data));
        add_walk(&mut app, tap, Some(speeds));
        let mut run = Run {
            app,
            server,
            ms,
            wire,
            frames: 0,
            character,
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
            assert!(self.frames < 20_000, "the run finishes");
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
        let wp = {
            let w = self.app.world().resource::<BridgeResource>().0.world();
            w.units
                .iter()
                .find(|(k, u)| k.unit_type == 2 && u.position.is_some())
                .map(|(k, _)| *k)
                .expect("the waypoint in the model")
        };
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

    fn server_level(&self) -> Option<u32> {
        app_support::with(&self.server, |l| {
            let (p, _) = single_player::local_player(&l.host().game)?;
            let g = &mut l.host_mut().game;
            let room = g.game.lists.unit(p)?.room()?;
            g.events.action.hooks().drlg.level_id(&g.game, room)
        })
    }
}

/// The NPC menu row of `kind`: its index among the box's selectable rows.
fn menu_row_index(rows: &[Option<OptionKind>], kind: Option<OptionKind>) -> usize {
    rows.iter().position(|r| *r == kind).expect("the row")
}

#[test]
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

    // 2. Talk to Akara: the interact sender (the preview's arrival).
    let akara = run
        .find(1, u32::from(d2_sim::world::npc::class::AKARA))
        .expect("Akara in the model");
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
    // The box is the spec box above Akara (`menus.md` §2.6), as ui 8.
    assert!(run.ui_open(8), "the menu is ui 8 (panels-2.md §14)");
    let p = app_support::npc_menu_row(&run.app, menu_row_index(&rows, Some(OptionKind::Trade)));
    run.click(p);
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
}

/// The combat tables of the field leg, on the server before the join (the
/// synthetic game has none; as `app_play_monster_ai.rs` and
/// `d2_sim::bench_fixtures::combat`): Attack (skill 0, do function 1),
/// monster class 0 (killable, 100 experience), the vitals tables (level 2
/// at 100 experience, 5 stat points a level), the stat table, and the
/// AnimData records of the sorceress' A1 and the monster's death.
fn install_combat(sim: &mut single_player::Sim) {
    use d2_formats::animdata::{self, AnimRecord};
    use d2_sim::bench_fixtures::combat as fx;
    let s = &mut sim.events.action.sys;
    s.stats = d2_sim::stats::StatLists::new(d2_sim::bench_fixtures::stat_data());
    let mut skills = fx::skills();
    let mut attack = fx::skill_rec();
    attack.srvdofunc = 1;
    attack.anim = 7;
    attack.range = 1;
    skills.skills[0] = attack;
    skills.skills.truncate(1);
    let combat = fx::combat_tables();
    s.hooks.tables = Arc::new(d2_sim::wiring::action::ActionTables {
        missiles: vec![fx::arrow()],
        skills,
        combat,
        levels: vec![fx_blank(); 150],
        skill_modes: vec![[0; 8]],
    });
    let mut a = fx::anim_data();
    for (name, event) in [(b"SOA1HTH\0", Some(2usize)), (b"M0DTHTH\0", None)] {
        let mut events = [0u8; animdata::EVENTS];
        if let Some(i) = event {
            events[i] = 1;
        }
        let len = name.iter().position(|&b| b == 0).unwrap();
        a.buckets[animdata::hash(&name[..len])].push(AnimRecord {
            name: *name,
            frames: 6,
            speed: 256,
            events,
        });
    }
    s.hooks.anim_data = Some(Arc::new(a));
    s.hooks.vitals = Some(Arc::new(fx::vitals()));
    // The server's animation names follow the client art's rules
    // (`app/anim_names.rs`): the sorceress `SO`, monster class 0 `M0`.
    let player_modes = [
        "DT", "NU", "WL", "RN", "GH", "TN", "TW", "A1", "A2", "BL", "SC", "TH", "KK", "S1", "S2",
        "S3", "S4", "DD", "SQ",
    ];
    let monster_modes = [
        "DT", "NU", "WL", "GH", "A1", "A2", "BL", "SC", "S1", "S2", "S3", "S4", "DD", "KB", "SQ",
        "RN",
    ];
    s.hooks.x.looks = Some(Arc::new(UnitLooks {
        player_tokens: vec![code(b"SO"); 7],
        player_modes: player_modes.iter().map(|m| code(m.as_bytes())).collect(),
        monster_modes: monster_modes.iter().map(|m| code(m.as_bytes())).collect(),
        monsters: [(
            0,
            d2_client::world_view::unit_assets::MonsterRow {
                token: code(b"M0"),
                base_w: None,
                composite_death: false,
            },
        )]
        .into(),
        ..UnitLooks::default()
    }));
    s.data = d2_sim::units::hooks::UnitData {
        monsters: vec![d2_sim::units::hooks::MonsterInfo {
            enabled: true,
            aidel: [15, 15, 15],
            moves: 0,
        }],
        ..Default::default()
    };
}

fn fx_blank<T: d2_data::tables::Record>() -> T {
    T::decode(&vec![0u8; T::SIZE])
}

/// The field leg: out of town by the waypoint, a monster two sub-tiles
/// away, the left skill (Attack) on it kills it (C→S 0x06), the kill's
/// experience levels the player up, and a stat point is spent (C→S 0x3A).
#[test]
fn the_field_leg_kills_levels_up_and_spends_a_point() {
    use d2_sim::stats::stat;
    let mut run = Run::start_with(install_combat);
    run.check("join with the combat tables");
    // The client's monster row for class 0 (a class without a row is
    // ignored, `client/msg-units.md` §1.2 r2).
    {
        use d2_client::bridge::world::{MonsterClass, MonsterSetup};
        let mut rows = single_player::synthetic_unit_rows();
        rows.monsters[0] = Some(MonsterClass {
            setup: Some(MonsterSetup {
                is_att: true,
                is_sel: true,
                ..MonsterSetup::default()
            }),
            ..MonsterClass::default()
        });
        run.bridge().set_unit_rows(rows);
    }
    {
        let w = run.app.world().resource::<BridgeResource>().0.world();
        let skills = w.local().and_then(|p| p.skills.as_ref());
        eprintln!("skills: {skills:?}");
    }
    run.waypoint_to_cold_plains();
    let (p, m, guid) = app_support::with(&run.server, |l| {
        use d2_sim::missiles::unit_flag as flags;
        use d2_sim::units::lifecycle::AllocRequest;
        use d2_sim::units::UnitType;
        let sim = &mut l.host_mut().game;
        let (p, _) = single_player::local_player(sim).expect("joined");
        let a = &mut sim.events.action;
        let (px, py) = a.sys.hooks.path_position(p);
        let room = sim.game.lists.unit(p).and_then(|e| e.room());
        let req = AllocRequest {
            ty: UnitType::Monster,
            class: 0,
            room,
            add: true,
            fixed_guid: None,
            mode: 1,
            allied: false,
        };
        let m = a
            .with(&mut sim.game, |g, v| v.allocate(g, &req, px + 2, py))
            .expect("monster");
        a.with(&mut sim.game, |_, v| {
            v.set_base(p, 19, 1000);
            v.set_base(p, 21, 2560);
            v.set_base(p, 22, 2560);
            v.set_base(m, stat::LEVEL, 1);
            v.set_base(m, 13, 100);
            v.set_base(m, stat::MAXHP, 256);
            v.set_base(m, stat::HITPOINTS, 256);
        });
        // Monster init's unit flags (`monsters/init.md`: |= 0x0A at
        // init, 0x04 for `isAtt`); 0x02 is the target check's
        // "targetable" (`use.md` §5.3 step 2).
        a.sys.units.get_mut(m).unwrap().flags |=
            0x02 | flags::IS_VALID_TARGET | flags::CAN_BE_ATTACKED;
        (p, m, sim.game.lists.unit(m).unwrap().guid)
    });
    run.step(10);
    run.check("a monster in the field");
    let mut attack = vec![0x06, 1, 0, 0, 0];
    attack.extend_from_slice(&guid.to_le_bytes());
    run.bridge().send_bytes(&attack).unwrap();
    let life = move |r: &mut Run| {
        app_support::with(&r.server, move |l| {
            let sim = &mut l.host_mut().game;
            let a = &mut sim.events.action;
            a.with(&mut sim.game, |_, v| v.stat(m, stat::HITPOINTS))
        })
    };
    for _ in 0..60 {
        if life(&mut run) <= 0 {
            break;
        }
        run.step(1);
    }
    let (log, errors) = app_support::with(&run.server, |l| {
        let h = l.host_mut().game.events.action.hooks();
        (h.x.skills.log.clone(), format!("{:?}", h.errors))
    });
    let list = app_support::with(&run.server, move |l| {
        let h = l.host_mut().game.events.action.hooks();
        let lst = format!("{:?}", h.skill_lists.get(&p));
        let xlog = format!("{:?}", h.x.log.iter().rev().take(8).collect::<Vec<_>>());
        let mode = l.host().game.events.action.sys.units.get(p).map(|u| u.mode);
        format!("{lst} mode {mode:?} xlog {xlog}")
    });
    let refused = format!("{:?}", run.wire.lock().unwrap().refused);
    assert!(
        life(&mut run) <= 0,
        "the monster died: {log:?} {errors} list {list} refused {refused}"
    );
    run.step(20);
    run.check("kill");
    let level = |r: &mut Run, s: u16| {
        app_support::with(&r.server, move |l| {
            l.host().game.events.action.sys.stats.unit_base(p, s, 0)
        })
    };
    assert_eq!(level(&mut run, stat::LEVEL), 2, "level up on the kill");
    let points = level(&mut run, 4);
    assert_eq!(points, 5, "stat points");
    run.bridge().send_bytes(&[0x3A, 0, 0]).unwrap();
    run.step(4);
    run.check("spend a stat point");
    assert_eq!(level(&mut run, 4), 4);
}

/// The five-act fixture set (`test_fixtures::acts::all_acts`) with the
/// patches `play_native.rs` makes, so the play app's live build
/// (`GameData::Live`, every act created) runs on it.
fn five_act_set() -> test_fixtures::synth::Synthetic {
    let mut s = test_fixtures::acts::all_acts();
    let pad = |s: &mut test_fixtures::synth::Synthetic, txt: &str, col: &str, n: usize| {
        let m = s.tables.files.get_mut(txt).unwrap();
        let id = m.columns.iter().position(|c| c == col).unwrap();
        let mut row = m.rows[0].clone();
        while m.rows.len() < n {
            row[id] = format!("pad{}", m.rows.len());
            m.rows.push(row.clone());
        }
    };
    // The fix-up of monstats record 707 reads monmode 15; the hireling
    // table reads pettype row 7.
    pad(&mut s, "monmode.txt", "name", 16);
    pad(&mut s, "pettype.txt", "pet type", 8);
    // `WaypointTables::live` needs a waypoint object (operatefn 23,
    // initfn 17).
    // `itemstatcost` to 1.14d's 359 rows (the base set has the first 16;
    // a stat past them, e.g. 67 velocitypercent, is not kept, so a player
    // runs at the 25 % floor, `pathing.md` §8.1 r2). Made-up names, 10
    // send / save bits, 32 for the 32-bit ones the play path reads.
    {
        let f = s.tables.file("itemstatcost.txt");
        let have = f.rows.len();
        for i in have..359 {
            let name = format!("stat{i}");
            let bits = "32";
            s.tables.row(
                "itemstatcost",
                &[
                    ("stat", name.as_str()),
                    ("send bits", bits),
                    ("save bits", bits),
                    ("csvbits", bits),
                ],
            );
        }
    }
    // Single player reads `monlvl`'s `L-` columns (`monsters/init.md`
    // §8.1, game type 3); the base set fills `L-AC` and `L-HP` only, so
    // a monster had no experience, to-hit or damage: the `L-` values are
    // the plain columns'.
    {
        let f = s.tables.files.get_mut("monlvl.txt").unwrap();
        let col = |f: &test_fixtures::synth::TxtFile, n: &str| {
            f.columns
                .iter()
                .position(|c| c.eq_ignore_ascii_case(n))
                .unwrap()
        };
        for (from, to) in [("XP", "L-XP"), ("TH", "L-TH"), ("DM", "L-DM")] {
            let (a, b) = (col(f, from), col(f, to));
            for r in f.rows.iter_mut() {
                r[b] = r[a].clone();
            }
        }
    }
    // A Town Portal scroll: type `scro` (1.14d row 22, `items::ty::SCRO`),
    // code `tsc` (the item-use stand-in of REC-117), one in the
    // sorceress' start items.
    s.tables.row(
        "itemtypes",
        &[
            ("code", "scro"),
            ("equiv1", "misc"),
            ("storepage", "misc"),
            ("body", "0"),
            ("normal", "1"),
            ("rarity", "3"),
        ],
    );
    s.tables.row(
        "misc",
        &[
            ("code", "tsc"),
            ("namestr", "tsc"),
            ("type", "scro"),
            ("level", "0"),
            ("invwidth", "1"),
            ("invheight", "1"),
            ("stackable", "0"),
            ("cost", "25"),
            ("spawnable", "1"),
            ("useable", "1"),
        ],
    );
    for (c, v) in [("item3", "tsc"), ("item3count", "1")] {
        s.tables.set("charstats", 1, c, v);
    }
    // `objects` row 59, the town portal (1.14d's TownPortal; operate 15,
    // init 11 as the synthetic game's, `object-functions.tsv`), the rows
    // before it padded.
    {
        let f = s.tables.files.get_mut("objects.txt").unwrap();
        let col = |f: &test_fixtures::synth::TxtFile, n: &str| {
            f.columns
                .iter()
                .position(|c| c.eq_ignore_ascii_case(n))
                .unwrap()
        };
        let name = col(f, "Name");
        let blank = vec![String::new(); f.columns.len()];
        while f.rows.len() < 59 {
            let mut r = blank.clone();
            r[name] = format!("pad{}", f.rows.len());
            f.rows.push(r);
        }
        let mut r = blank.clone();
        for (c, v) in [
            ("Name", "TownPortal"),
            ("Token", "TP"),
            ("SizeX", "1"),
            ("SizeY", "1"),
            ("FrameCnt1", "15"),
            ("Selectable0", "1"),
            ("Selectable1", "1"),
            ("OperateRange", "4"),
            ("OperateFn", "15"),
            ("InitFn", "11"),
        ] {
            r[col(f, c)] = v.into();
        }
        f.rows.truncate(59);
        f.rows.push(r);
    }
    // `itemtypes` on 1.14d's row numbers: the game reads some types by
    // row (`items::ty`: gold 4, play 7, weap 45, armo 50, misc 52); the
    // base set's rows are in its own order (gold on row 7, the ear type),
    // so a gold drop was made as an ear and failed (`Create(NotPlayer)`).
    // The other rows are placeholders; potn and blad take rows no `ty`
    // constant names.
    {
        let f = s.tables.files.get_mut("itemtypes.txt").unwrap();
        let code = f
            .columns
            .iter()
            .position(|c| c.eq_ignore_ascii_case("code"))
            .unwrap();
        let old = std::mem::take(&mut f.rows);
        let at = |c: &str| match c {
            "" => Some(0),
            "tors" => Some(3),
            "gold" => Some(4),
            "potn" => Some(9),
            "blad" => Some(30),
            "weap" => Some(45),
            "armo" => Some(50),
            "misc" => Some(52),
            "scro" => Some(22),
            _ => None,
        };
        let mut rows: Vec<Vec<String>> = (0..75)
            .map(|i| {
                let mut r = vec![String::new(); f.columns.len()];
                r[code] = format!("x{i:02}");
                r
            })
            .collect();
        for r in old {
            let i = at(&r[code]).unwrap_or_else(|| panic!("itemtypes {:?}", r[code]));
            rows[i] = r;
        }
        f.rows = rows;
    }
    // Every kill drops all of its treasure class (below).
    {
        let f = s.tables.files.get_mut("treasureclassex.txt").unwrap();
        let col = |f: &test_fixtures::synth::TxtFile, n: &str| {
            f.columns
                .iter()
                .position(|c| c.eq_ignore_ascii_case(n))
                .unwrap()
        };
        let (tc, nodrop, picks) = (col(f, "Treasure Class"), col(f, "NoDrop"), col(f, "Picks"));
        let probs: Vec<usize> = (1..=4).map(|i| col(f, &format!("Prob{i}"))).collect();
        for r in f.rows.iter_mut().filter(|r| r[tc] == "Synth Act 1") {
            // Negative picks: each item its `Prob` times (`treasure.md`):
            // the gold pile, a weapon, an armor and a potion, once each.
            r[nodrop] = String::new();
            r[picks] = "-4".into();
            for &p in &probs {
                r[p] = "1".into();
            }
        }
    }
    // `playerclass` row 7 with the empty code, as 1.14d's ("Expansion"):
    // an empty `class` cell (`itemtypes`, a `playerclass.code` link)
    // links to it, the no-class value 7 (`CLASS_NONE`); without it the
    // link fails (255) and no class may equip the type.
    s.tables.row("playerclass", &[("code", "")]);
    // The short blade a level-1 character can wear (the base set's asks
    // level 2: the equip check fails and it stays on the cursor).
    {
        let f = s.tables.files.get_mut("weapons.txt").unwrap();
        let col = |f: &test_fixtures::synth::TxtFile, n: &str| {
            f.columns
                .iter()
                .position(|c| c.eq_ignore_ascii_case(n))
                .unwrap()
        };
        let (code, req) = (col(f, "code"), col(f, "levelreq"));
        for r in f.rows.iter_mut().filter(|r| r[code] == "sb1") {
            r[req] = "1".into();
        }
    }
    // Attack as 1.14d's `skills.txt` row 0 has it: `anim` / `monanim` A1
    // and the Attack do function (`srvdofunc` 1, `bodies.md` §4.1).
    for (c, v) in [("anim", "A1"), ("monanim", "A1"), ("srvdofunc", "1")] {
        s.tables.set("skills", 0, c, v);
    }
    // AnimData for the player classes (the base set has the monsters'
    // only): every `plrtype` token × `plrmode` token, bare hands, 8 frames
    // at speed 256; A1 with its attack event (1) on frame 4.
    for i in 0..7 {
        for mode in ["DT", "NU", "WL", "RN", "GH", "TN", "TW", "A1"] {
            s.animdata.push(test_fixtures::animdata::Anim {
                name: format!("C{i}{mode}HTH"),
                frames: 8,
                speed: 256,
                events: if mode == "A1" {
                    vec![(4, 1)]
                } else {
                    Vec::new()
                },
            });
        }
    }
    // Monsters spawn in the outdoor levels (as `act1_stream.rs`'s
    // `spawning_act1`): every monster row with a `Rarity` is `isSpawn`.
    let f = s.tables.file("monstats.txt");
    let col = |n: &str| {
        f.columns
            .iter()
            .position(|c| c.eq_ignore_ascii_case(n))
            .unwrap()
    };
    let (rarity, id) = (col("Rarity"), col("Id"));
    let rows: Vec<usize> = (0..f.rows.len())
        .filter(|&i| !f.rows[i][rarity].is_empty() && !f.rows[i][id].is_empty())
        .collect();
    for i in rows {
        s.tables.set("monstats", i, "isSpawn", "1");
    }
    let o = s.tables.files.get_mut("objects.txt").unwrap();
    for (col, v) in [("OperateFn", "23"), ("InitFn", "17")] {
        let c = o.columns.iter().position(|x| x == col).unwrap();
        o.rows[0][c] = v.into();
    }
    s
}

/// The five-act install on disk (in the test's own temp folder).
fn five_act_install(name: &str) -> std::path::PathBuf {
    let dir = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("smoke-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    test_fixtures::install::build(&dir, &five_act_set()).unwrap();
    dir
}

/// The play app's live build (`GameData::select` on an install, every
/// act created, `LevelSource::live`) runs on the five-act fixture install
/// (it stopped at `Drlg(UnknownLevel(40))` on the Act I install, F2).
#[test]
fn the_live_play_game_builds_on_the_five_act_install() {
    let dir = five_act_install("build");
    let data = GameData::select(Some(&dir), false).unwrap();
    assert!(matches!(data, GameData::Live(_)));
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
        assert!(
            test_fixtures::host::cheb(drawn, p) <= 2,
            "run leg to ({x}, {y}): the drawn player stands where the server's does: {drawn:?} / {p:?}"
        );
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

    /// Run legs into level `to` (from `from`) until the server player is
    /// there.
    fn walk_into(&mut self, from: u32, to: u32) {
        use test_fixtures::host::{border_goals, cheb, LEG};
        let goals = border_goals(self.level_rect(from), self.level_rect(to), self.pos());
        for g in goals {
            for _ in 0..30 {
                if self.server_level() == Some(to) {
                    return;
                }
                let p = self.pos();
                let before = cheb(p, g);
                self.leg((
                    p.0 + (g.0 - p.0).clamp(-LEG, LEG),
                    p.1 + (g.1 - p.1).clamp(-LEG, LEG),
                ));
                if cheb(self.pos(), g) >= before {
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
fn the_live_run() {
    let dir = five_act_install("live");
    let data = GameData::select(Some(&dir), false).unwrap();
    let mut run = Run::start_on(data.clone(), |_| {});
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
    assert_eq!(worn, Some(weapon), "the weapon is worn again");

    // Level up: more beasts until level 2 (100 experience, 63 a beast),
    // searching deeper into the Blood Moor when none is near.
    let rect = run.level_rect(2);
    let centre = (
        (rect.x + rect.w / 2) * test_fixtures::host::SUB,
        (rect.y + rect.h / 2) * test_fixtures::host::SUB,
    );
    let mut unreachable = Vec::new();
    for _ in 0..30 {
        if run.player_stat(12) >= 2 {
            break;
        }
        let near = run.monsters_near();
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
    let (points, skills) = (run.player_stat(4), run.player_stat(5));
    assert!(
        points >= 5 && skills >= 1,
        "points {points}, skill points {skills}"
    );
    // Spend: strength + 1 (C→S 0x3A stat 0, count − 1 = 0), Firebolt
    // (skill 2, the sorceress' class skill) + 1 (0x3B).
    let str0 = run.player_stat(0);
    run.bridge().send_bytes(&[0x3A, 0, 0]).unwrap();
    run.bridge().send_bytes(&[0x3B, 2, 0]).unwrap();
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
        firebolt.is_some_and(|l| l.contains("skill: 2,")),
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

    // Save and exit, as `play::run` does on every way out (`save::share`'s
    // save: the running game read, applied over the base, written with
    // the install's `.d2s` tables), then load the file and join again.
    let GameData::Live(live) = &data else {
        unreachable!("a live run")
    };
    let path = dir.join("Smoke.d2s");
    let before = run.snapshot();
    {
        use d2_client::app::save;
        let base = save::base_save(&run.character);
        let got = app_support::with(&run.server, |l| save::read_live(&mut l.host_mut().game));
        let d2s = save::apply_live(&base, &got.expect("the live read"), 0);
        save::write_file(&path, &d2s, &live.save).expect("the save written");
    }
    drop(run);
    let character = single_player::load_character(&data, &path, 0).expect("the save loads");
    let mut run = Run::start_as(data.clone(), character, |_| {});
    run.check("join the saved character");
    let after = run.snapshot();
    assert_eq!(after, before, "the loaded character is the saved one");
}

impl Run {
    /// The beasts (monster class 0, the fixture's weakest) of the model
    /// with a position, nearest to the player first: a bare-handed new
    /// character kills them in a few dozen swings.
    fn monsters_near(&self) -> Vec<UnitKey> {
        let w = self.app.world().resource::<BridgeResource>().0.world();
        // The server player's position (the harness's choice of target;
        // the client's own reading can lag, see the handoff).
        let (px, py) = self.pos();
        let (px, py) = (px as u16, py as u16);
        let mut v: Vec<(i32, UnitKey)> = w
            .units
            .iter()
            .filter(|(k, u)| {
                k.unit_type == 1 && u.class == 0 && u.position.is_some() && !u.is_dead()
            })
            .map(|(k, u)| {
                let (x, y) = u.position.unwrap();
                let d = (i32::from(x) - i32::from(px))
                    .abs()
                    .max((i32::from(y) - i32::from(py)).abs());
                (d, *k)
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
    /// stats, gold, its class skills' levels and its items (code, mode,
    /// body location), sorted.
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
        (stats, skills, items)
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
        for _ in 0..150 {
            match self.monster_life(key.guid) {
                Some(l) if l > 0 => {}
                _ => return true,
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
        }
        matches!(self.monster_life(key.guid), None | Some(..=0))
    }
}

/// A run into a wall: the drawn player stops where the server's player
/// stops. The fixture tiles have no walls, so the test stamps one (wall
/// bit 0x1, `drlg/rooms.md` §10) into the server's and the client's grids
/// alike, as 1.14d builds both from the same tiles. The walk prediction
/// steps the player's own path over the client grids (`ClientPath`,
/// REC-277 (d)); a straight line would run on through the wall.
#[test]
fn a_blocked_run_is_drawn_where_the_server_stops() {
    let dir = five_act_install("wall");
    let data = GameData::select(Some(&dir), false).unwrap();
    let mut run = Run::start_on(data, |_| {});
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
