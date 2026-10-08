// Spec: specs/world/npc.md (§2, §3), specs/world/quests.md (§6.2, §7.2, §7.3), specs/client/msg-ui.md (§16); preview fills: docs/PLAN.md decisions D1–D3, docs/handoff/q-quests.md
//! The quest path of the play preview headless, wired as `d2-client play`
//! wires it (synthetic fixtures only, as in `app_play_npc.rs`): a left
//! click on Akara walks there and sends C→S 0x13; the server starts the
//! interaction (S→C 0x27, 0x29, 0x28), the client answers C→S 0x2F and
//! the quest message 0x31, and the server starts the Den of Evil.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use d2_client::app::palette::{add_act_palettes, ActPalettes};
use d2_client::app::play::{
    add_client_data, add_game, add_preview, add_walk, predict_link, send_create_game_for,
};
use d2_client::app::single_player::{self, GameData};
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
fn play_app(ms: &Arc<AtomicU32>, wire: &Arc<Mutex<Wire>>) -> App {
    let data = GameData::Synthetic;
    let character = single_player::new_character("sorceress", "Test").unwrap();
    let (link, _) = single_player::start_with(
        data.clone(),
        single_player::DEFAULT_SEED,
        character.clone(),
        StepClock(ms.clone()),
    )
    .unwrap();
    let source = Arc::new(files());
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default()))
        .init_asset::<Image>()
        .init_resource::<ButtonInput<MouseButton>>();
    let link = Recorder {
        inner: Box::new(link),
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
    app
}

/// The client's monster rows: Akara is an `npc` and `interact` class,
/// attackable-flagged so the pick takes her (`isAtt`, `bridge/click.rs`).
fn set_npc_rows(app: &mut App) {
    app.world_mut()
        .resource_mut::<BridgeResource>()
        .0
        .set_unit_rows(single_player::synthetic_unit_rows());
}

/// The screen point of Akara (the one monster in the model), as the
/// click's camera sees it.
fn akara_on_screen(app: &App) -> (u32, Point) {
    let w = app.world().resource::<BridgeResource>().0.world();
    let (key, u) = w
        .units
        .iter()
        .find(|(k, u)| k.unit_type == 1 && u.class == 148 && u.position.is_some())
        .expect("Akara in the model");
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

/// The player's quest record the client holds (`[0x007C0D43]`).
fn client_record(app: &App) -> [u8; 96] {
    let ui = app.world().non_send::<WorldViewUi>();
    ui.original.as_ref().unwrap().more().client_quest
}

/// Talks to Akara: click, walk, C→S 0x13, then the server's reply and the
/// client's 0x2F / 0x31.
fn talk_to_akara(app: &mut App, ms: &AtomicU32, wire: &Arc<Mutex<Wire>>) -> u32 {
    let guid = open_akara_menu(app, ms, wire);
    // Cancel (the last row of the NPC menu box) ends the chat: C→S 0x30.
    let p = app_support::npc_menu_row(app, 2);
    click(app, ms, p);
    step(app, ms, 3);
    guid
}

/// Clicks Akara and waits for her menu (S→C 0x28).
fn open_akara_menu(app: &mut App, ms: &AtomicU32, wire: &Arc<Mutex<Wire>>) -> u32 {
    let (guid, at) = akara_on_screen(app);
    assert!(
        (0..800).contains(&at.x) && (0..560).contains(&at.y),
        "Akara is on screen: {at:?}"
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
        "C→S 0x13 on Akara: {:?}",
        ids(wire)
    );
    step(app, ms, 6);
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

// Covers: specs/world/npc.md §2, §3; specs/world/quests.md §7.2, §7.3
#[test]
fn talking_to_akara_twice_starts_the_den_of_evil() {
    let ms = Arc::new(AtomicU32::new(1000));
    let wire = Arc::new(Mutex::new(Wire::default()));
    let mut app = play_app(&ms, &wire);
    assert_eq!(client_record(&app)[2] & 0x04, 0, "Den not started yet");
    // First talk: the server starts the interaction (S→C 0x27 / 0x29 /
    // 0x28), the client answers C→S 0x2F and the quest message 0x31 of
    // the list (Akara's introduction, string 12), then closes the chat.
    let guid = talk_to_akara(&mut app, &ms, &wire);
    let mut chat = vec![0x2F];
    chat.extend_from_slice(&1u32.to_le_bytes());
    chat.extend_from_slice(&guid.to_le_bytes());
    assert!(
        wire.lock().unwrap().sent.contains(&chat),
        "C→S 0x2F: {:?}",
        ids(&wire)
    );
    assert_eq!(quest_messages(&wire), [(guid, 12)]);
    let mut end = vec![0x30];
    end.extend_from_slice(&1u32.to_le_bytes());
    end.extend_from_slice(&guid.to_le_bytes());
    assert!(wire.lock().unwrap().sent.contains(&end), "C→S 0x30");
    // Second talk: the list now carries the Den of Evil (message 64,
    // `quest-messages.tsv`); the server starts the quest on it
    // (`quests.md` §7.3): slot 1 bit 2 in the record S→C 0x28 sends.
    talk_to_akara(&mut app, &ms, &wire);
    assert_eq!(quest_messages(&wire), [(guid, 12), (guid, 64)]);
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

// Covers: specs/world/quests.md §6.2; specs/world/quests-status.md §3
#[test]
fn the_quest_log_shows_the_started_den_of_evil() {
    let ms = Arc::new(AtomicU32::new(1000));
    let wire = Arc::new(Mutex::new(Wire::default()));
    let mut app = play_app(&ms, &wire);
    // Before Akara: the log opens, the server answers with every status 0.
    press_q(&mut app, &ms);
    assert!(wire.lock().unwrap().sent.contains(&vec![0x40]), "C→S 0x40");
    {
        let ui = app.world().non_send::<WorldViewUi>();
        assert!(
            ui.original.as_ref().unwrap().is_open(0x0F),
            "quest log open"
        );
    }
    let before = log_rows(&app);
    assert_eq!(before.len(), 6, "six Act I rows: {before:?}");
    assert!(
        before.iter().all(|r| r.2 == 0),
        "nothing started: {before:?}"
    );
    press_q(&mut app, &ms);
    // Akara twice (introduction, then the Den of Evil), then the log.
    talk_to_akara(&mut app, &ms, &wire);
    talk_to_akara(&mut app, &ms, &wire);
    press_q(&mut app, &ms);
    let rows = log_rows(&app);
    let den = rows.iter().find(|r| r.0 == 1).expect("the Den of Evil row");
    assert_eq!(den.2, 1, "status 1 (started): {rows:?}");
    assert_eq!(den.1, IconState::InProgress);
}

// Covers: specs/ui/menus.md §2; specs/world/npc.md §3
#[test]
fn akaras_menu_offers_talk_trade_and_cancel() {
    let ms = Arc::new(AtomicU32::new(1000));
    let wire = Arc::new(Mutex::new(Wire::default()));
    let mut app = play_app(&ms, &wire);
    let guid = open_akara_menu(&mut app, &ms, &wire);
    let menu = {
        let ui = app.world().non_send::<WorldViewUi>();
        ui.original.as_ref().unwrap().npc_menu()
    };
    let menu = menu.expect("the NPC menu is open after the 0x28");
    assert_eq!(menu.guid, guid);
    let kinds: Vec<_> = menu.rows.iter().map(|r| r.kind).collect();
    use d2_client::ui::layout::OptionKind::{Talk, Trade};
    assert_eq!(kinds, [Some(Talk), Some(Trade), None]);
    // The automatic chat close is gone: the chat stays open.
    assert!(
        !ids(&wire).contains(&0x30),
        "no C→S 0x30 yet: {:?}",
        ids(&wire)
    );
    // Talk closes the box and opens the topic box (`panels-2.md` §14.9,
    // `messages.md` §6 r3).
    let p = app_support::npc_menu_row(&app, 0);
    click(&mut app, &ms, p);
    step(&mut app, &ms, 2);
    let ui = app.world().non_send::<WorldViewUi>();
    assert!(ui.original.as_ref().unwrap().npc_menu().unwrap().talking);
    // The topic box's cancel ends the talk: Akara's menu is built again
    // (§14.8: flag 1).
    let p = app_support::npc_topic_cancel(&app);
    click(&mut app, &ms, p);
    step(&mut app, &ms, 2);
    assert_eq!(app_support::npc_menu_len(&app), 3, "the menu is back");
    // Trade: C→S 0x38 action 1 [GUID] and the menu closes.
    let p = app_support::npc_menu_row(&app, 1);
    click(&mut app, &ms, p);
    step(&mut app, &ms, 3);
    let mut want = vec![0x38, 1, 0, 0, 0];
    want.extend_from_slice(&guid.to_le_bytes());
    want.extend_from_slice(&0u32.to_le_bytes());
    assert!(
        wire.lock().unwrap().sent.contains(&want),
        "C→S 0x38 trade: {:?}",
        ids(&wire)
    );
    let ui = app.world().non_send::<WorldViewUi>();
    assert!(ui.original.as_ref().unwrap().npc_menu().is_none());
}

// Covers: specs/world/npc.md §3
#[test]
fn leaving_the_menu_sends_the_chat_end() {
    let ms = Arc::new(AtomicU32::new(1000));
    let wire = Arc::new(Mutex::new(Wire::default()));
    let mut app = play_app(&ms, &wire);
    let guid = talk_to_akara(&mut app, &ms, &wire);
    let mut want = vec![0x30, 1, 0, 0, 0];
    want.extend_from_slice(&guid.to_le_bytes());
    assert!(
        wire.lock().unwrap().sent.contains(&want),
        "C→S 0x30: {:?}",
        ids(&wire)
    );
}
