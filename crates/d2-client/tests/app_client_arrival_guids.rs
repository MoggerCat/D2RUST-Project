// Spec: specs/client/model.md (§5 rules 6.1–6.4), specs/monsters/population.md (§11.7)
//! The client-made units of the Act 1 arrival on the user's install,
//! against the 1.14d recording `traces/client/a1-arrival-creations-ama.jsonl`
//! (format client-creations-1: Game.exe under Wine,
//! `record_client_creations.py --auto ScnAma --seed 1234`). All 205
//! creations come in the room pass of the client update that first sees
//! the town's rooms active: each room's critters, then its client
//! presets; the GUIDs count up from 2 (q-fix-p6-client-arrival-guids).

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use d2_client::app::play::{add_live_client, LiveClient};
use d2_client::app::save;
use d2_client::app::single_player::{self, Character, GameData};
use d2_client::bridge::BridgeResource;
use d2_server::adapters::character::LoadContext;
use d2_server::seams::Clock;

mod app_support;

use app_support::{Server, SharedLink};

struct StepClock(Arc<AtomicU32>);

impl Clock for StepClock {
    fn now_ms(&mut self) -> u32 {
        self.0.load(Ordering::SeqCst)
    }
}

fn step(app: &mut App, ms: &AtomicU32) {
    app.update();
    ms.fetch_add(40, Ordering::SeqCst);
}

/// One client creation: (GUID, class, type, x, y).
type Creation = (u32, u32, u8, u16, u16);

/// The `create` records of the recording (and the `think0` GUIDs with
/// their T).
fn recorded() -> (Vec<Creation>, Vec<(u32, i64)>) {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../traces/client/a1-arrival-creations-ama.jsonl"
    );
    let text = std::fs::read_to_string(path).unwrap();
    let mut creates = Vec::new();
    let mut thinks = Vec::new();
    for line in text.lines() {
        let v: serde_json::Value = serde_json::from_str(line).unwrap();
        let n = |k: &str| v[k].as_i64().unwrap();
        match v["k"].as_str().unwrap() {
            "header" => assert_eq!(v["format"], "client-creations-1"),
            "create" => creates.push((
                n("guid") as u32,
                n("cls") as u32,
                n("type") as u8,
                n("x") as u16,
                n("y") as u16,
            )),
            "think0" => thinks.push((n("guid") as u32, n("T"))),
            k => panic!("unknown record {k}"),
        }
    }
    (creates, thinks)
}

// Covers: specs/client/model.md §5 r6; specs/monsters/population.md §11.7
#[test]
#[ignore = "real data: needs D2_GAME_DIR (tools/realdata-gate.sh)"]
fn the_arrival_room_pass_makes_the_recorded_client_units() {
    let ms = Arc::new(AtomicU32::new(1000));
    let new = single_player::new_character("amazon", "ScnAma").unwrap();
    let character = Character::Save(
        Box::new(save::base_save(&new)),
        LoadContext {
            difficulty: 0,
            map_seed_applies: false,
        },
    );
    let data = app_support::game_data();
    let GameData::Live(live) = data.clone();
    let speeds = single_player::walk_speeds(&data, &character).unwrap();
    let (link, started) =
        single_player::start_with(data, 1234, character.clone(), StepClock(ms.clone())).unwrap();
    let server: Server<StepClock> = Arc::new(Mutex::new(link));
    let mut app = App::new();
    app.insert_resource(d2_client::bridge::mirror::ScriptedClock(ms.clone()));
    app.add_plugins((MinimalPlugins, AssetPlugin::default()))
        .init_asset::<Image>()
        .init_resource::<ButtonInput<MouseButton>>();
    add_live_client(
        &mut app,
        Box::new(SharedLink(server.clone())),
        LiveClient {
            data: &live,
            request: &character,
            start_flags: None,
            prices: started.prices,
            speeds,
            hardcore: false,
            automap_files: None,
            gpu: false,
        },
    )
    .unwrap();
    let set_c = |app: &App| -> Vec<Creation> {
        let w = app.world().resource::<BridgeResource>().0.world();
        w.objclient
            .set_c
            .iter()
            .map(|(k, u)| {
                let (x, y) = u.position.unwrap_or((0, 0));
                (k.guid, u.class, k.unit_type, x, y)
            })
            .collect()
    };
    let mut frames = 0;
    while set_c(&app).is_empty() {
        step(&mut app, &ms);
        frames += 1;
        assert!(frames < 400, "no client unit made");
    }
    let mut got = set_c(&app);
    got.sort();
    let (want, thinks) = recorded();
    for (i, (g, w)) in got.iter().zip(&want).enumerate() {
        assert_eq!(g, w, "client creation {i}");
    }
    assert_eq!(got.len(), want.len());
    // Every critter reads T = 0 at its first AI call (`think0`).
    let w = app.world().resource::<BridgeResource>().0.world();
    for (guid, t) in thinks {
        let key = d2_client::bridge::world::UnitKey::new(1, guid);
        let u = w.objclient.set_c.get(&key).expect("recorded critter");
        let d2_client::bridge::world::KindData::Monster(d) = &u.kind else {
            panic!("critter {guid} is no monster");
        };
        assert_eq!(d.first_think.map(i64::from), Some(t), "critter {guid}");
    }
}
