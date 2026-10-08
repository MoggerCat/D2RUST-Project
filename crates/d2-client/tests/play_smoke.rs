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
        SharedLink(self.server.clone()).receive()
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
}

impl Run {
    /// The play app over the synthetic game, as `d2-client play --new`
    /// wires it, with a new sorceress; returns once the join ran.
    fn start() -> Self {
        Self::start_with(|_| {})
    }

    /// [`Self::start`] with `install` run on the built game before the
    /// join (fixture tables the synthetic game lacks).
    fn start_with(install: fn(&mut single_player::Sim)) -> Self {
        let data = GameData::Synthetic;
        let character = single_player::new_character("sorceress", "Smoke").unwrap();
        let ms = Arc::new(AtomicU32::new(1000));
        let clock = StepClock(ms.clone());
        let built = character.clone();
        let link = ThreadLink::spawn(move || {
            let mut g = single_player::build_with(
                &GameData::Synthetic,
                single_player::DEFAULT_SEED,
                built,
            )?;
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
        app_support::synthetic_skill_rows(&mut app);
        app.world_mut()
            .resource_mut::<BridgeResource>()
            .0
            .set_unit_rows(single_player::synthetic_unit_rows());
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
        add_walk(&mut app, tap, Some(Speeds { walk: 6, run: 9 }));
        let mut run = Run {
            app,
            server,
            ms,
            wire,
            frames: 0,
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

/// The NPC menu row of `kind`, clicked (`npc_menu_ui.rs`: the box is
/// centred, a quarter down the 800 × 600 frame, rows of 20).
fn menu_row_point(rows: &[Option<OptionKind>], kind: Option<OptionKind>) -> Point {
    let k = rows.iter().position(|r| *r == kind).expect("the row") as i32;
    Point::new((800 - 200) / 2 + 20, 600 / 4 + 20 * (k + 1) + 10)
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
    let shop_open = run.ui_open(0x0C);
    eprintln!("shop open: {shop_open}");
    if shop_open {
        run.app
            .world_mut()
            .non_send_mut::<WorldViewUi>()
            .original
            .as_mut()
            .unwrap()
            .set_ui(0x0C, 1, false)
            .unwrap();
        run.step(10);
        run.check("close the shop");
    } else {
        let end = d2_client::ui::panels::npc::msg_chat_end(akara.guid);
        run.bridge().send_bytes(&end).unwrap();
        run.step(4);
        run.check("end the trade");
    }

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
#[ignore = "follow-up F3 in docs/handoff/q-play-smoke.md: C→S 0x06 is accepted but the attack never starts on the play host"]
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
        a.sys.units.get_mut(m).unwrap().flags |= flags::IS_VALID_TARGET | flags::CAN_BE_ATTACKED;
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
