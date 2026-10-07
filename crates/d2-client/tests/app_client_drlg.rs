// Spec: specs/client/model.md (§9, §11, §12), specs/render/composition.md (§4)
//! The client DRLG in the play mode's wiring, headless: `add_game` +
//! `add_client_data` over a link that delivers the single-player join in
//! the order of `model.md` §11 rule 3 (0x01, 0x03, 0x59 at (0, 0), 0x0B,
//! 0x07, 0x15, 0x04; the in-process server's session code does not send
//! the join yet, `docs/HANDOFF.md` §2 step 4). The client builds its own
//! act DRLG from 0x03 and the rooms 0x07 brings in sight; the local
//! player's level is the level of its room.

use std::collections::VecDeque;

use bevy::prelude::*;
use d2_client::app::play::{add_client_data, add_game};
use d2_client::app::single_player::{self, GameData};
use d2_client::bridge::link::{LinkError, Pumped, SendQueue, Sent, ServerLink};
use d2_client::bridge::world::{ClientTables, ClientWorld};
use d2_client::bridge::{Bridge, BridgeResource};
use d2_client::rules::OpenMode;
use d2_client::world_view::WorldViewState;
use d2_proto::PROTOCOL_VERSION;

/// Delivers one chunk list per pump (each pump a tick).
struct Script(VecDeque<Vec<Vec<u8>>>, Vec<Vec<u8>>);

impl ServerLink for Script {
    fn protocol_version(&self) -> u32 {
        PROTOCOL_VERSION
    }
    fn send(&mut self, _: SendQueue, _: &[u8]) -> Result<Sent, LinkError> {
        Ok(Sent::Queued)
    }
    fn pump(&mut self) -> Result<Pumped, LinkError> {
        self.1 = self.0.pop_front().unwrap_or_default();
        Ok(Pumped { ticked: true })
    }
    fn receive(&mut self) -> Vec<Vec<u8>> {
        std::mem::take(&mut self.1)
    }
}

fn hex(s: &str) -> Vec<u8> {
    s.split_whitespace()
        .map(|b| u8::from_str_radix(b, 16).unwrap())
        .collect()
}

/// 0x59 for player 1 ("werwer", class 1) at (0, 0).
fn assign_player() -> Vec<u8> {
    let mut b = hex("59 01 00 00 00 01 77 65 72 77 65 72");
    b.resize(0x1A, 0);
    b
}

/// The join of `model.md` §11 rule 3 with 0x03 `load_act`, the 0x07
/// `reveal` and the 0x15 `place`; frame 1 receives everything up to
/// 0x15, frame 2 the 0x04.
fn join(load_act: &str, reveal: &str, place: &str) -> Script {
    let frame1 = vec![
        hex("01 00 04 00 10 00 01 00"),
        hex(load_act),
        assign_player(),
        hex("0b 00 01 00 00 00"),
        hex(reveal),
        hex(place),
    ];
    Script(VecDeque::from([frame1, vec![hex("04")]]), Vec::new())
}

fn check_join(w: &ClientWorld, rejected: usize) -> u16 {
    assert_eq!(rejected, 0);
    assert!(w.in_game);
    let act = w.act.expect("0x03 received");
    let d = w.drlg.as_ref().expect("client DRLG built");
    assert_eq!((d.drlg.act, d.drlg.init_seed), (act.act, act.init_seed));
    assert!(d.drlg.on_client);
    let own = w
        .local_room()
        .expect("the local player is in an active room");
    assert_eq!(w.player_level(), Some(own.level));
    own.level
}

// Covers: specs/client/model.md §12 r1, §11 r3, §11 r5
#[test]
fn the_join_builds_the_client_drlg_in_the_app() {
    let data = GameData::Synthetic;
    // Cold Plains (level 3) is one 8 × 8-tile room at tile (0, 0).
    let link = join(
        "03 00 01 00 00 00 01 00 00 00 00 00",
        "07 00 00 00 00 03",
        "15 00 01 00 00 00 05 00 05 00 01",
    );
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default()))
        .init_asset::<Image>()
        .init_resource::<ButtonInput<MouseButton>>();
    add_game(&mut app, Box::new(link), false).unwrap();
    add_client_data(
        &mut app,
        single_player::client_drlg_source(&data),
        single_player::client_level_rows(&data),
    );
    // No original UI here: the open mode it would hand over with every
    // panel closed (`ui/panels.md` §4.2), so the world view can place.
    app.world_mut()
        .resource_mut::<WorldViewState>()
        .feed
        .set_ui_open_mode(OpenMode::new(0).unwrap());
    app.update();
    app.update();
    let b = &app.world().resource::<BridgeResource>().0;
    let w = b.world();
    let level = check_join(w, b.log().rejected.len());
    assert_eq!(u32::from(level), single_player::COLD_PLAINS);
    assert_eq!(
        w.active_rooms.as_ref().map(|r| r.len()),
        Some(1),
        "the one room of the level"
    );
    // The feed answers BlankScreen from the player's level's row (the
    // synthetic rows have BlankScreen 0).
    let state = app.world().resource::<WorldViewState>();
    assert!(!state.feed.blank_screen(w).unwrap());
}

/// The recorded join of `client/model.md` §Test vectors (recording
/// `20261006-022633`): 0x03 seq 142 (act 0, init seed 0x103888C4), 0x07
/// seq 144 (level 1, tile (0x3A0, 0x388)), 0x15 seq 154 (player 1 to
/// (4673, 4548)). Expect the client DRLG of the user's tables to have a
/// level-1 room of origin tile (928, 904) holding the player: level 1.
///
/// `D2_GAME_DIR=<install> cargo test -p d2-client --test app_client_drlg -- --ignored`
// Covers: specs/client/model.md §12 r1, §9 r1, §11 r3; specs/render/composition.md §4
#[test]
#[ignore = "needs original game files in D2_GAME_DIR"]
fn the_recorded_join_on_the_install() {
    use d2_client::app::palette::{act_palette_path, ActPalettes};

    let dir = std::env::var("D2_GAME_DIR").expect("D2_GAME_DIR must be set");
    let data = GameData::select(Some(std::path::Path::new(&dir)), false).unwrap();
    let GameData::Live(live) = &data else {
        unreachable!("a game dir selects live data")
    };
    let link = join(
        "03 00 c4 88 38 10 01 00 61 d1 e0 9f",
        "07 a0 03 88 03 01",
        "15 00 01 00 00 00 41 12 c4 11 01",
    );
    let mut bridge = Bridge::new(link).unwrap();
    bridge.set_drlg_source(Some(single_player::client_drlg_source(&data)));
    bridge.set_tables(ClientTables {
        levels: single_player::client_level_rows(&data),
        ..ClientTables::default()
    });
    bridge.frame().unwrap();
    bridge.frame().unwrap();
    let w = bridge.world();
    let level = check_join(w, bridge.log().rejected.len());
    assert_eq!(level, 1);
    let own = w.local_room().unwrap();
    assert_eq!((own.x0, own.y0), (928 * 5, 904 * 5));
    // Every act palette is in the archives and is a valid `pal.pl2`.
    let palettes = ActPalettes::live(&live.archives).unwrap();
    for a in 0..5 {
        d2_client::scene::present_palette(palettes.of(a))
            .unwrap_or_else(|e| panic!("{}: {e}", act_palette_path(a)));
    }
}
