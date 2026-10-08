// Spec: specs/world/quests-act1.md §10.5; specs/world/quests.md §4.4, §5, §6.2; specs/world/npc.md §2, §3; specs/monsters/init.md §14.3; preview fills: docs/handoff/q-a1-bloodraven.md
//! Act I's Sisters' Burial Grounds in the play preview, headless over the
//! synthetic game (as `app_play_quests.rs`): Kashya gives the quest
//! (message 81), the player enters the Burial Grounds (quest event 3),
//! Blood Raven is there with her quest chain link, her death completes the
//! quest (event 8, the updater's timer) and Kashya's reward runs
//! (message 92).

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use d2_client::app::palette::{add_act_palettes, ActPalettes};
use d2_client::app::play::{
    add_client_data, add_game, add_preview, add_walk, predict_link, send_create_game_for,
};
use d2_client::app::server_thread::ThreadLink;
use d2_client::app::single_player::{self, GameData, Link};
use d2_client::app::ui::{add_original_ui_with, UiParts};
use d2_client::assets::path::MemorySource;
use d2_client::bridge::hover;
use d2_client::bridge::link::{LinkError, Pumped, SendQueue, Sent, ServerLink};
use d2_client::bridge::mirror::DynLink;
use d2_client::bridge::predict::Speeds;
use d2_client::bridge::BridgeResource;
use d2_client::controls::Action;
use d2_client::rules::camera::{moving_to_client, Camera, FrameSize, OpenMode};
use d2_client::rules::unit_composite::code;
use d2_client::ui::layout::Screen;
use d2_client::ui::original::{FontMeasure, OriginalUi, UiConfig, CHARACTER_FONTS};
use d2_client::ui::quest_log::IconState;
use d2_client::ui::{font_info, ActionId, Point, PointerButton, UiEvent};
use d2_client::world_view::tile_assets::TileAssets;
use d2_client::world_view::unit_assets::UnitLooks;
use d2_client::world_view::walk::PreviewWalk;
use d2_client::world_view::WorldViewUi;
use d2_server::seams::Clock;

mod app_support;

/// What crossed the link: the C→S messages.
#[derive(Default)]
struct Wire {
    sent: Vec<Vec<u8>>,
}

/// The server thread's link, shared with the test body.
type Handle = Arc<Mutex<ThreadLink<Link<StepClock>>>>;

struct Shared(Handle);

impl ServerLink for Shared {
    fn protocol_version(&self) -> u32 {
        self.0.lock().unwrap().protocol_version()
    }
    fn send(&mut self, q: SendQueue, msg: &[u8]) -> Result<Sent, LinkError> {
        self.0.lock().unwrap().send(q, msg)
    }
    fn pump(&mut self) -> Result<Pumped, LinkError> {
        self.0.lock().unwrap().pump()
    }
    fn receive(&mut self) -> Vec<Vec<u8>> {
        self.0.lock().unwrap().receive()
    }
}

/// A link that records what crosses it.
struct Recorder {
    inner: DynLink,
    wire: Arc<Mutex<Wire>>,
}

impl ServerLink for Recorder {
    fn protocol_version(&self) -> u32 {
        self.inner.protocol_version()
    }
    fn send(&mut self, queue: SendQueue, msg: &[u8]) -> Result<Sent, LinkError> {
        self.wire.lock().unwrap().sent.push(msg.to_vec());
        self.inner.send(queue, msg)
    }
    fn pump(&mut self) -> Result<Pumped, LinkError> {
        self.inner.pump()
    }
    fn receive(&mut self) -> Vec<Vec<u8>> {
        self.inner.receive()
    }
}

struct StepClock(Arc<AtomicU32>);

impl Clock for StepClock {
    fn now_ms(&mut self) -> u32 {
        self.0.load(Ordering::SeqCst)
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
/// component 1 `TR`. `OYTRlitTNhth` is the one player component file
/// name read as a DC6 (`unit-composite.md` §6 r2), so a DC6 fixture
/// draws the player.
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

fn step(app: &mut App, ms: &AtomicU32, n: usize) {
    for _ in 0..n {
        app.update();
        ms.fetch_add(40, Ordering::SeqCst);
    }
}

fn queue(app: &mut App, e: UiEvent) {
    app.world_mut()
        .non_send_mut::<WorldViewUi>()
        .queue
        .0
        .push(e);
}

/// The play app over the synthetic game, joined, with a left skill.
fn play_app(ms: &Arc<AtomicU32>, wire: &Arc<Mutex<Wire>>) -> (App, Handle) {
    let data = GameData::Synthetic;
    let character = single_player::new_character("sorceress", "Test").unwrap();
    let (link, _) = single_player::start_with(
        data.clone(),
        single_player::DEFAULT_SEED,
        character.clone(),
        StepClock(ms.clone()),
    )
    .unwrap();
    let handle: Handle = Arc::new(Mutex::new(link));
    let source = Arc::new(files());
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default()))
        .init_asset::<Image>()
        .init_resource::<ButtonInput<MouseButton>>();
    let link = Recorder {
        inner: Box::new(Shared(handle.clone())),
        wire: wire.clone(),
    };
    let (link, tap) = predict_link(Box::new(link));
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
    set_npc_rows(&mut app);
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
    step(&mut app, ms, 10);
    // The synthetic join sends no skill list: S→C 0x94 + 0x23 (as in
    // `app_play_e2e.rs`).
    let guid = app
        .world()
        .resource::<BridgeResource>()
        .0
        .world()
        .local()
        .expect("local player")
        .key
        .guid;
    let mut msgs = vec![0x94, 1];
    msgs.extend_from_slice(&guid.to_le_bytes());
    msgs.extend_from_slice(&[0, 0, 1]);
    msgs.extend_from_slice(&[0x23, 0]);
    msgs.extend_from_slice(&guid.to_le_bytes());
    msgs.extend_from_slice(&[1, 0, 0]);
    msgs.extend_from_slice(&u32::MAX.to_le_bytes());
    app.world_mut()
        .resource_mut::<BridgeResource>()
        .0
        .receive_chunk(&msgs)
        .unwrap();
    (app, handle)
}

/// The client's monster rows: Akara is an `npc` and `interact` class,
/// attackable-flagged so the pick takes her (`isAtt`, `bridge/click.rs`).
fn set_npc_rows(app: &mut App) {
    app.world_mut()
        .resource_mut::<BridgeResource>()
        .0
        .set_unit_rows(single_player::synthetic_unit_rows());
}

/// The screen point of the NPC of `class` (a monster in the model), as the
/// click's camera sees it.
fn npc_on_screen(app: &App, class: u16) -> (u32, Point) {
    let w = app.world().resource::<BridgeResource>().0.world();
    let (key, u) = w
        .units
        .iter()
        .find(|(k, u)| k.unit_type == 1 && u.class == u32::from(class) && u.position.is_some())
        .expect("the NPC in the model");
    let at = app
        .world()
        .resource::<PreviewWalk>()
        .predict
        .position()
        .expect("predicted player");
    let cam = Camera::new(
        FrameSize::D2RS,
        OpenMode::NONE,
        moving_to_client(at.0, at.1),
        (0, 0),
    );
    let (x, y) = hover::feet(&cam, u.position.unwrap());
    (key.guid, Point::new(x, y - 20))
}

fn click(app: &mut App, ms: &AtomicU32, at: Point) {
    for button in [true, false] {
        let e = if button {
            UiEvent::Press {
                button: PointerButton::Left,
                at,
            }
        } else {
            UiEvent::Release {
                button: PointerButton::Left,
                at,
            }
        };
        queue(app, e);
    }
    step(app, ms, 2);
}

fn ids(wire: &Arc<Mutex<Wire>>) -> Vec<u8> {
    wire.lock().unwrap().sent.iter().map(|m| m[0]).collect()
}

/// Clicks the NPC of `class` and waits for its menu (S→C 0x28).
fn open_menu(app: &mut App, ms: &AtomicU32, wire: &Arc<Mutex<Wire>>, class: u16) -> u32 {
    let (guid, at) = npc_on_screen(app, class);
    assert!(
        (0..800).contains(&at.x) && (0..560).contains(&at.y),
        "NPC {class} is on screen: {at:?}"
    );
    click(app, ms, at);
    let mut want = vec![0x13, 1, 0, 0, 0];
    want.extend_from_slice(&guid.to_le_bytes());
    for _ in 0..200 {
        step(app, ms, 1);
        if wire.lock().unwrap().sent.contains(&want) {
            break;
        }
    }
    assert!(
        wire.lock().unwrap().sent.contains(&want),
        "C→S 0x13 on NPC {class}: {:?}",
        ids(wire)
    );
    for _ in 0..300 {
        let ui = app.world().non_send::<WorldViewUi>();
        if ui.original.as_ref().unwrap().npc_menu().is_some() {
            break;
        }
        step(app, ms, 1);
    }
    step(app, ms, 2);
    guid
}

/// Talks to the NPC: the menu's Talk row (R800: box x 300, y 150, rows
/// from y 170, 20 high), then Cancel ends the chat.
fn talk(app: &mut App, ms: &AtomicU32, wire: &Arc<Mutex<Wire>>, class: u16) -> u32 {
    let guid = open_menu(app, ms, wire, class);
    click(app, ms, Point::new(400, 170 + 5));
    step(app, ms, 4);
    click(app, ms, Point::new(400, 170 + 20 * 2 + 5));
    step(app, ms, 3);
    guid
}

/// The C→S 0x31 messages sent so far, as (GUID, message).
fn quest_messages(wire: &Arc<Mutex<Wire>>) -> Vec<(u32, u32)> {
    wire.lock()
        .unwrap()
        .sent
        .iter()
        .filter(|m| m[0] == 0x31)
        .map(|m| {
            (
                u32::from_le_bytes(m[1..5].try_into().unwrap()),
                u32::from_le_bytes(m[5..9].try_into().unwrap()),
            )
        })
        .collect()
}

fn press_q(app: &mut App, ms: &AtomicU32) {
    queue(
        app,
        UiEvent::Action(ActionId(Action::ToggleQuests.index() as u16)),
    );
    step(app, ms, 3);
}

fn log_rows(app: &App) -> Vec<(u8, IconState, u8)> {
    let ui = app.world().non_send::<WorldViewUi>();
    ui.original
        .as_ref()
        .unwrap()
        .quest_rows(0, true)
        .iter()
        .map(|r| (r.row.quest, r.row.icon, r.row.shown))
        .collect()
}

type Server = Handle;

/// Runs `f` on the game inside the server thread.
fn on_server<R: Send + 'static>(
    h: &Server,
    f: impl FnOnce(&mut single_player::Sim) -> R + Send + 'static,
) -> R {
    h.lock()
        .unwrap()
        .with(move |l| f(&mut l.host_mut().game))
        .unwrap()
}

/// The server's quest record of `chain`: (state, status).
fn quest(h: &Server, chain: u8) -> (u8, u8) {
    on_server(h, move |s| {
        let r = s.world.quests.record(chain).unwrap();
        (r.state, r.status)
    })
}

/// The Den of Evil done at game entry: its sequence opens chain 2 at state
/// 1 (`quests-act1.md` §10.1, as the server test `burial_grounds_through_
/// every_state` sets it up).
fn den_done(h: &Server) {
    on_server(h, |s| {
        let (p, _) = single_player::local_player(s).expect("joined");
        s.world.rest.quests.get_mut(&p).unwrap().flags[0].set(1, 0);
        let r = s.world.quests.record_mut(1).unwrap();
        r.not_intro = false;
        r.active = false;
        let r = s.world.quests.record_mut(2).unwrap();
        r.not_intro = true;
        r.active = true;
        r.state = 1;
    });
}

/// The server's level of the local player.
fn level(h: &Server) -> Option<u32> {
    on_server(h, |s| {
        let (p, _) = single_player::local_player(s)?;
        let room = s.game.lists.unit(p)?.room()?;
        s.events.action.hooks().drlg.level_id(&s.game, room)
    })
}

// Covers: specs/world/quests-act1.md §10.5 r7; specs/world/quests.md §7.3
#[test]
fn kashya_gives_the_sisters_burial_grounds() {
    let ms = Arc::new(AtomicU32::new(1000));
    let wire = Arc::new(Mutex::new(Wire::default()));
    let (mut app, h) = play_app(&ms, &wire);
    den_done(&h);
    assert_eq!(quest(&h, 2), (1, 0));
    // The first talk is her introduction (message 24), the second the quest.
    let kashya = talk(&mut app, &ms, &wire, 150);
    assert_eq!(quest_messages(&wire), [(kashya, 24)]);
    talk(&mut app, &ms, &wire, 150);
    let msgs = quest_messages(&wire);
    assert!(
        msgs.contains(&(kashya, 81)),
        "Kashya's message 81: {msgs:?} {:?} {}",
        ids(&wire),
        on_server(&h, |s| format!(
            "{:?} {:?}",
            s.world.state.errors, s.world.rest.log
        )) + &on_server(&h, |s| {
            let (p, _) = single_player::local_player(s).unwrap();
            let mut out = format!(
                "player {:?} lists {:?} rec {} interact {:?} sent {:?}",
                s.events.action.sys.hooks.path_position(p),
                s.world.state.lists.keys().collect::<Vec<_>>(),
                s.world.npc.record(150).is_some(),
                s.events.action.sys.units.get(p).unwrap().interact.get(),
                s.world.rest.sent.len()
            );
            for u in s.game.lists.units_of_type(d2_sim::units::UnitType::Monster) {
                out += &format!(
                    " m{} {:?}",
                    s.events.action.sys.units.get(u).unwrap().class,
                    s.events.action.sys.hooks.path_position(u)
                );
            }
            out
        })
    );
    assert_eq!(quest(&h, 2).0, 2, "the quest is started");
    // The quest log: Act I row 2 shows the quest started.
    press_q(&mut app, &ms);
    let rows = log_rows(&app);
    let row = rows
        .iter()
        .find(|r| r.0 == 2)
        .expect("the Burial Grounds row");
    assert_eq!(row.2, 1, "status 1 (started): {rows:?}");
    assert_eq!(row.1, IconState::InProgress);
}

// Covers: specs/world/quests-act1.md §10.5 r3, §10.5 r4, §10.5 r5, §10.5 r7; specs/world/quests.md §4.4, §5; specs/monsters/init.md §14.3
#[test]
fn blood_raven_dies_and_kashya_pays() {
    let ms = Arc::new(AtomicU32::new(1000));
    let wire = Arc::new(Mutex::new(Wire::default()));
    let (mut app, h) = play_app(&ms, &wire);
    den_done(&h);
    let kashya = talk(&mut app, &ms, &wire, 150);
    talk(&mut app, &ms, &wire, 150);
    assert_eq!(quest(&h, 2).0, 2, "started");
    // Through the Blood Moor's second entrance to the Burial Grounds.
    let entrance = app
        .world()
        .resource::<BridgeResource>()
        .0
        .world()
        .units
        .iter()
        .find(|(k, u)| {
            k.unit_type == d2_client::bridge::world::TILE
                && u.class == d2_client::app::synthetic_burial::BLOOD_MOOR_TO_BURIAL
        })
        .map(|(k, _)| *k)
        .expect("the Burial Grounds entrance reached the client");
    app.world_mut()
        .resource_mut::<BridgeResource>()
        .0
        .interact(entrance)
        .unwrap();
    for _ in 0..300 {
        if level(&h) == Some(d2_client::app::synthetic_burial::BURIAL_GROUNDS) {
            break;
        }
        step(&mut app, &ms, 1);
    }
    assert_eq!(
        level(&h),
        Some(d2_client::app::synthetic_burial::BURIAL_GROUNDS),
        "arrived"
    );
    step(&mut app, &ms, 30);
    // Event 3 (b = 17): state 3.
    assert_eq!(quest(&h, 2).0, 3, "entered the Burial Grounds");
    // Blood Raven stands there with her chain link.
    let raven = on_server(&h, |s| {
        let m = s.game.lists.units_of_type(d2_sim::units::UnitType::Monster);
        let r = m.into_iter().find(|&u| {
            s.events
                .action
                .sys
                .units
                .get(u)
                .is_some_and(|r| r.class == 267)
        });
        let linked = r.is_some_and(|r| {
            d2_sim::wiring::economy::QuestRest::quest_chain(&mut s.world.rest, r)
                .is_some_and(|c| c.0 == [2])
        });
        r.filter(|_| linked)
            .map(|r| s.game.lists.unit(r).unwrap().guid)
    });
    let raven = raven.expect("Blood Raven with chain 2");
    {
        let w = app.world().resource::<BridgeResource>().0.world();
        assert!(
            w.units
                .iter()
                .any(|(k, u)| k.guid == raven && u.class == 267),
            "Blood Raven reached the client"
        );
    }
    // She dies by the player's hand: event 8 → state 4.
    on_server(&h, move |s| {
        let (p, _) = single_player::local_player(s).unwrap();
        let r = s
            .game
            .lists
            .find_unit(d2_sim::units::UnitType::Monster, raven)
            .unwrap();
        s.events.action.combat(&mut s.game, |cv, _| {
            d2_sim::wiring::action::reaction::kill(cv, r, p)
        });
    });
    step(&mut app, &ms, 40);
    assert_eq!(quest(&h, 2).0, 4, "Blood Raven is dead");
    // The updater's timer (period 15, run every 20th frame) tells the
    // client the quest is done (S→C 0x5D status 3).
    step(&mut app, &ms, 20 * 17);
    assert_eq!(quest(&h, 2).1, 3, "completed-now status");
    let _ = kashya;
}

// Covers: specs/world/quests-act1.md §10.5 r7; specs/world/quests.md §6.2
#[test]
fn kashyas_reward_runs_after_blood_raven() {
    let ms = Arc::new(AtomicU32::new(1000));
    let wire = Arc::new(Mutex::new(Wire::default()));
    let (mut app, h) = play_app(&ms, &wire);
    den_done(&h);
    // Blood Raven is dead and the player was near (J3: 2.1, event 8's 2.13).
    on_server(&h, |s| {
        let (p, _) = single_player::local_player(s).expect("joined");
        let f = &mut s.world.rest.quests.get_mut(&p).unwrap().flags[0];
        f.set(2, 1);
        f.set(2, 13);
        s.world.quests.record_mut(2).unwrap().state = 4;
    });
    let kashya = talk(&mut app, &ms, &wire, 150);
    talk(&mut app, &ms, &wire, 150);
    let msgs = quest_messages(&wire);
    assert!(
        msgs.contains(&(kashya, 92)),
        "Kashya's message 92: {msgs:?}"
    );
    assert_eq!(quest(&h, 2), (5, 13), "rewarded");
    // The reward marks one of Kashya's mercenaries hired; the synthetic
    // game has no hireling tables, so the unit itself is not created
    // (`NoHirelingTables`, docs/handoff/q-a1-bloodraven.md).
    let hired = on_server(&h, |s| {
        let hire = s.world.npc.record(150).and_then(|r| r.hire.as_ref());
        hire.map(|h| h.slots.iter().filter(|s| s.hired).count())
    });
    assert_eq!(hired, Some(1), "the mercenary is Kashya's reward");
}
