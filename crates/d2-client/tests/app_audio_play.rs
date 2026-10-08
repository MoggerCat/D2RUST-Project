// Spec: specs/audio/sound-table.md (§4 r5, §6.4 r2, §8.1 r1)
//! The original sound layer on the play path: the app's single-player
//! game (synthetic tables) in a headless Bevy `App`, the audio frame with
//! the original parts ([`AudioParts::original`], a small `sounds.txt`),
//! and sound requests that ask the model a question it cannot answer
//! yet (the line test, the client seed). Such a question is reported and
//! answered neutrally; it never fails the frame (Bevy's default error
//! handler panics, so a failed audio frame stopped play on the first unit
//! sound of the real install).

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use d2_client::app::play::{add_game, send_create_game};
use d2_client::app::single_player::{self, GameData, DEFAULT_SEED};
use d2_client::app::sound::{AudioParts, GameAudio};
use d2_client::assets::path::MemorySource;
use d2_client::audio::driver::SoundRequest;
use d2_client::audio::sound_table::SoundTableData;
use d2_client::bridge::BridgeResource;
use d2_client::rules::OpenMode;
use d2_client::world_view::{UiSounds, WorldViewState};
use d2_data::txt::TxtTable;
use d2_server::seams::Clock;

mod app_support;
use app_support::SharedLink;

struct StepClock(Arc<AtomicU32>);

impl Clock for StepClock {
    fn now_ms(&mut self) -> u32 {
        self.0.load(Ordering::SeqCst)
    }
}

/// Ids 0–3: id 2 heads a group of 2 (a variant draw on the client seed).
fn table() -> SoundTableData {
    let s = "Sound\tIndex\tFileName\tVolume\tGroup Size\tBlock 1\tBlock 2\tBlock 3\r\n\
             none\t0\tnone.wav\t0\t0\t-1\t-1\t-1\r\n\
             a\t1\ta.wav\t255\t0\t-1\t-1\t-1\r\n\
             b\t2\tb.wav\t255\t2\t-1\t-1\t-1\r\n\
             c\t3\tc.wav\t255\t0\t-1\t-1\t-1\r\n";
    let e = "Handle\tIndex\tSong\r\nx\t0\t0\r\n";
    let st = TxtTable::parse("sounds.txt", s.as_bytes()).unwrap();
    let et = TxtTable::parse("soundenviron.txt", e.as_bytes()).unwrap();
    SoundTableData::from_txt(&st, &et).unwrap()
}

// Covers: specs/audio/sound-table.md §6.4 r2, §4 r5, §4 r6
#[test]
fn a_pending_sound_input_does_not_stop_play() {
    let ms = Arc::new(AtomicU32::new(1000));
    let (link, _) =
        single_player::start(GameData::Synthetic, DEFAULT_SEED, StepClock(ms.clone())).unwrap();
    let server = Arc::new(Mutex::new(link));
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default()))
        .init_asset::<Image>()
        .init_resource::<ButtonInput<MouseButton>>();
    add_game(&mut app, Box::new(SharedLink(server.clone())), true).unwrap();
    app_support::synthetic_skill_rows(&mut app);
    app.world_mut()
        .resource_mut::<WorldViewState>()
        .feed
        .set_ui_open_mode(OpenMode::new(0).unwrap());
    app.insert_resource(GameAudio::new(AudioParts::original(
        Arc::new(MemorySource::default()),
        table(),
    )));
    app.init_resource::<UiSounds>();
    send_create_game(&mut app).unwrap();
    // The join: the local player enters the model.
    let mut local = None;
    for _ in 0..40 {
        ms.fetch_add(40, Ordering::SeqCst);
        app.update();
        local = app
            .world()
            .resource::<BridgeResource>()
            .0
            .world()
            .local_player;
        if local.is_some() {
            break;
        }
    }
    let p = local.expect("the local player joined");
    // A unit sound on the player (its line test is pending) and a grouped
    // UI sound (its variant draw needs the client seed).
    app.world_mut().resource_mut::<UiSounds>().0.extend([
        SoundRequest::UnitRequest { id: 1, unit: p },
        SoundRequest::Ui(2),
    ]);
    for _ in 0..3 {
        ms.fetch_add(40, Ordering::SeqCst);
        app.update();
    }
    let audio = app.world().resource::<GameAudio>();
    assert!(
        !audio.stats.pending.is_empty(),
        "the pending questions are reported"
    );
    let ticks = app
        .world()
        .resource::<BridgeResource>()
        .0
        .world()
        .server_ticks;
    assert_eq!(
        audio.stats.presented, ticks as u32,
        "a sound tick per server tick"
    );
}
