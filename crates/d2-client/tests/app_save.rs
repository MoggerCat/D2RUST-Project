// Spec: specs/formats/d2s.md (§1, §7), specs/formats/d2s-load.md
//! Saving the played character (stitch-save): a save joins the synthetic
//! single-player game, the game's live values are written back by
//! `app::save`, and the written file loads again as the same character.
//! Preview behaviour is d2rs-own, unverified (rule 10).

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use d2_client::app::save::{self, SaveHandle};
use d2_client::app::single_player::{self, Character, DEFAULT_SEED};
use d2_client::bridge::link::{SendQueue, ServerLink};
use d2_formats::d2s::{self, Body, D2s, Header, ReadOptions, StatEntry, StatSave, Stats};
use d2_server::adapters::character::LoadContext;

mod app_support;

struct StepClock(Arc<AtomicU32>);

impl d2_server::seams::Clock for StepClock {
    fn now_ms(&mut self) -> u32 {
        self.0.load(Ordering::SeqCst)
    }
}

/// 32-bit stats, no items: enough for the preview's save.
struct Tables;

impl d2s::SaveTables for Tables {
    fn stat_save(&self, id: u16) -> Option<StatSave> {
        (id < 400).then_some(StatSave {
            bits: 32,
            param: 0,
            signed: true,
        })
    }
    /// A new character carries its start cube (REC-244): the entries
    /// are read with the synthetic item tables.
    fn item_entry_len(&self, buf: &[u8]) -> Result<usize, String> {
        let items = app_support::live().tables.item_tables().unwrap();
        d2_sim::items::bitstream::read::read_save_entry(buf, &items)
            .map(|e| e.len)
            .map_err(|e| e.to_string())
    }
}

fn synthetic_save() -> D2s {
    let mut header = Header::default();
    header.set_name(b"Rolf").unwrap();
    header.class = 1;
    header.level = 5;
    header.status = d2s::status::EXPANSION;
    header.towns[0] = 0x80;
    let stat = |id, value| StatEntry {
        id,
        layer: 0,
        value,
    };
    D2s {
        header,
        body: Some(Body {
            skills: vec![0; 30],
            stats: Stats::Bits(vec![
                stat(0, 25),
                stat(3, 22),
                stat(12, 5),
                stat(13, 1234),
                stat(14, 77),
            ]),
            ..Body::default()
        }),
    }
}

fn entries(s: &D2s) -> Vec<(u16, i32)> {
    let body = s.body.as_ref().unwrap();
    let mut v: Vec<_> = body
        .stats
        .entries()
        .iter()
        .map(|e| (e.id, e.value))
        .collect();
    v.sort();
    v
}

/// Runs the join of `character` on the synthetic game.
fn joined(
    character: Character,
) -> d2_client::app::server_thread::ThreadLink<single_player::Link<StepClock>> {
    let ms = Arc::new(AtomicU32::new(1000));
    let (mut link, _) = single_player::start_with(
        app_support::game_data(),
        DEFAULT_SEED,
        character.clone(),
        StepClock(ms.clone()),
    )
    .unwrap();
    link.send(
        SendQueue::System,
        &single_player::create_request_for(&character).encode(),
    )
    .unwrap();
    link.pump().unwrap();
    ms.fetch_add(40, Ordering::SeqCst);
    link.pump().unwrap();
    link.receive();
    link.send(SendQueue::System, &[0x6B]).unwrap();
    ms.fetch_add(40, Ordering::SeqCst);
    assert!(link.pump().unwrap().ticked);
    link.receive();
    link
}

fn handle(character: &Character, path: &std::path::Path) -> SaveHandle {
    let link = joined(character.clone());
    let (_shared, h) = save::share(
        link,
        save::base_save(character),
        Arc::new(Tables),
        None,
        path.into(),
    )
    .unwrap();
    h
}

fn temp(name: &str) -> std::path::PathBuf {
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("app-save-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

/// A save joins, is written, and reads back as the same character.
// Covers: specs/formats/d2s.md §1 r4, §7.1 r1; specs/formats/d2s-load.md §2
#[test]
#[ignore = "real data: needs D2_GAME_DIR (tools/realdata-gate.sh)"]
fn a_loaded_character_round_trips_through_the_save() {
    let base = synthetic_save();
    let character = Character::Save(
        Box::new(base.clone()),
        LoadContext {
            difficulty: 0,
            map_seed_applies: false,
        },
    );
    let dir = temp("round");
    let file = dir.join("Rolf.d2s");
    handle(&character, &file).save().unwrap();
    let bytes = std::fs::read(&file).unwrap();
    let opts = ReadOptions {
        expansion: true,
        game: None,
    };
    let back = d2s::read(&bytes, &opts, &Tables).unwrap();
    assert_eq!(back.header.name_bytes(), b"Rolf");
    assert_eq!(back.header.class, 1);
    assert_eq!(back.header.level, 5);
    // Every stat the file held comes back from the live player.
    let want = entries(&base);
    let got = entries(&back);
    for w in &want {
        assert!(got.contains(w), "{w:?} missing from {got:?}");
    }
    // Loading the written file again is the same character.
    let again = Character::Save(
        Box::new(back.clone()),
        LoadContext {
            difficulty: 0,
            map_seed_applies: false,
        },
    );
    let file2 = dir.join("Rolf2.d2s");
    handle(&again, &file2).save().unwrap();
    let back2 = d2s::read(&std::fs::read(&file2).unwrap(), &opts, &Tables).unwrap();
    assert_eq!(entries(&back2), got);
    assert_eq!(back2.header.name, back.header.name);
}

/// Saving again keeps the previous file as `.bak`.
// Covers: specs/formats/d2s.md §1 r4
#[test]
#[ignore = "real data: needs D2_GAME_DIR (tools/realdata-gate.sh)"]
fn a_second_save_keeps_a_backup() {
    let character = Character::Save(
        Box::new(synthetic_save()),
        LoadContext {
            difficulty: 0,
            map_seed_applies: false,
        },
    );
    let dir = temp("bak");
    let file = dir.join("Rolf.d2s");
    let h = handle(&character, &file);
    h.save().unwrap();
    let first = std::fs::read(&file).unwrap();
    h.save().unwrap();
    assert_eq!(std::fs::read(dir.join("Rolf.d2s.bak")).unwrap(), first);
}

/// `--new`: a fresh character is written to `<name>.d2s` and loads again.
// d2rs-own, unverified
#[test]
#[ignore = "real data: needs D2_GAME_DIR (tools/realdata-gate.sh)"]
fn a_new_character_is_written_to_its_own_file() {
    let character = single_player::new_character("barbarian", "Conan").unwrap();
    let dir = temp("new");
    let path = save::save_path(None, Some("Conan"), Some(&dir), None)
        .unwrap()
        .unwrap();
    assert_eq!(path, dir.join("Conan.d2s"));
    handle(&character, &path).save().unwrap();
    let opts = ReadOptions {
        expansion: single_player::GAME_SETUP.expansion,
        game: None,
    };
    let back = d2s::read(&std::fs::read(&path).unwrap(), &opts, &Tables).unwrap();
    assert_eq!(back.header.name_bytes(), b"Conan");
    assert_eq!(back.header.class, 4);
    // The name is taken now: a second `--new Conan` is refused.
    assert!(save::save_path(None, Some("Conan"), Some(&dir), None).is_err());
}

// d2rs-own, unverified
#[test]
fn paths_follow_the_flags() {
    use std::path::Path;
    assert_eq!(save::save_path(None, None, None, None).unwrap(), None);
    let f = Path::new("x/Y.d2s");
    assert_eq!(
        save::save_path(Some(f), None, None, None)
            .unwrap()
            .as_deref(),
        Some(f)
    );
    // Never inside the game install.
    let game = temp("game");
    std::fs::create_dir_all(&game).unwrap();
    assert!(save::save_path(None, Some("A"), Some(&game), Some(&game)).is_err());
    assert!(save::save_path(Some(&game.join("A.d2s")), None, None, Some(&game)).is_err());
}

/// The live values lay over the loaded ones: stats, quests, level.
// d2rs-own, unverified
#[test]
fn live_values_overlay_the_base() {
    let base = synthetic_save();
    let mut rec = [[0u8; 96]; 3];
    rec[0][3] = 9;
    let live = save::Live {
        stats: vec![(0, 30), (12, 6), (13, 5000), (14, 0), (15, 40)],
        quests: Some(rec),
        ..Default::default()
    };
    let out = save::apply_live(&base, &live, 777);
    let got = entries(&out);
    // Changed, added, and removed (a zero stat is not stored, §7.1).
    assert!(got.contains(&(0, 30)) && got.contains(&(15, 40)));
    assert!(got.contains(&(13, 5000)) && got.contains(&(3, 22)));
    assert!(!got.iter().any(|e| e.0 == 14));
    assert_eq!(out.header.level, 6);
    assert_eq!(out.header.save_time, 777);
    // The create time is never set by a game save (§2.2 r10, edge case 11).
    assert_eq!(out.header.create_time, base.header.create_time);
    assert_eq!(out.body.unwrap().quests.records, rec);
    // No live stats (a game with no stat table): the base stays as it is.
    let kept = save::apply_live(&base, &save::Live::default(), 1);
    assert_eq!(entries(&kept), entries(&base));
}

/// The waypoints of the loaded save are the played character's, and the
/// written file holds them again (q-save-full; fails when the load leaves
/// them unapplied and a new record is saved instead).
// Covers: specs/world/waypoints.md §3 r1, §3 r2
// d2rs-own, unverified
#[test]
#[ignore = "real data: needs D2_GAME_DIR (tools/realdata-gate.sh)"]
fn waypoints_round_trip_through_the_save() {
    let mut base = synthetic_save();
    // Waypoints 0 (default), 5 and 20 in Normal; 3 in Nightmare.
    let body = base.body.as_mut().unwrap();
    body.waypoints.records[0][2] |= 0x20;
    body.waypoints.records[0][4] |= 0x10;
    body.waypoints.records[1][2] |= 0x08;
    let want = body.waypoints.records;
    let character = Character::Save(
        Box::new(base),
        LoadContext {
            difficulty: 0,
            map_seed_applies: false,
        },
    );
    let dir = temp("waypoints");
    let file = dir.join("Rolf.d2s");
    // The base the live values lay over has the default waypoints, so
    // only the running game can supply the saved ones.
    let (_shared, h) = save::share(
        joined(character),
        synthetic_save(),
        Arc::new(Tables),
        None,
        file.clone(),
    )
    .unwrap();
    h.save().unwrap();
    let opts = ReadOptions {
        expansion: true,
        game: None,
    };
    let back = d2s::read(&std::fs::read(&file).unwrap(), &opts, &Tables).unwrap();
    assert_eq!(back.body.unwrap().waypoints.records, want);
}

/// The live items, skill levels and waypoints replace the base's;
/// `None` (no inventory model, no skill rows) keeps the base's.
// d2rs-own, unverified
#[test]
fn extra_values_overlay_the_body() {
    use d2_client::app::save_full::{apply_extra, Extra};
    let mut body = synthetic_save().body.unwrap();
    body.items = vec![d2s::ItemEntry { bytes: vec![1, 2] }];
    let mut wp = [[0u8; 16]; 3];
    wp[2][5] = 7;
    apply_extra(
        &mut body,
        &Extra {
            items: Some(vec![d2s::ItemEntry { bytes: vec![9] }]),
            skills: Some(vec![0, 3, 20]),
            waypoints: Some(wp),
            corpses: None,
        },
    );
    assert_eq!(body.items, [d2s::ItemEntry { bytes: vec![9] }]);
    assert_eq!(&body.skills[..4], [0, 3, 20, 0]);
    assert_eq!(body.skills.len(), 30);
    assert_eq!(body.waypoints.records, wp);
    // The corpse section follows the live corpse (`d2s.md` §8.3).
    let corpse = d2s::Corpse {
        items: vec![d2s::ItemEntry { bytes: vec![7] }],
        ..d2s::Corpse::default()
    };
    apply_extra(
        &mut body,
        &Extra {
            corpses: Some(vec![corpse.clone()]),
            ..Extra::default()
        },
    );
    assert_eq!(body.corpses, [corpse]);
    apply_extra(&mut body, &Extra::default());
    assert_eq!(
        body.corpses.len(),
        1,
        "no model: the section passes through"
    );
    assert_eq!(body.items.len(), 1);
    assert_eq!(body.skills[2], 20);
}

/// A new expansion character's file ends `6A 66 6B 66 00` like the game's
/// (§1 rule 2, §8.4 rule 4, §8.5 rule 3); a classic one has neither.
// Covers: specs/formats/d2s.md §8.4 r4, §8.5 r3
#[test]
fn a_new_character_writes_jf_and_kf_in_an_expansion_game() {
    let exp = single_player::GAME_SETUP.expansion;
    let base = save::base_save(&Character::New);
    let body = base.body.as_ref().unwrap();
    assert_eq!(body.hireling_items.is_some(), exp);
    assert_eq!(body.golem.is_some(), exp);
}

/// A loaded 0x5C–0x5E file is saved as 0x60 with the bit-field stats
/// (`d2s.md` §1 r6, §2.1, §7.1 r7: the game writes only 0x60); the file
/// writes and reads back as 0x60.
// Covers: specs/formats/d2s.md §1 r6, §7.1 r7
#[test]
fn an_old_version_save_is_written_as_0x60() {
    let mut base = synthetic_save();
    base.header.version = 0x5C;
    base.body.as_mut().unwrap().stats = Stats::Mask {
        mask: vec![0x0F, 0x10],
        values: vec![(0, 25), (1, 0), (3, 22), (12, 5)],
    };
    let live = save::Live {
        stats: vec![(0, 30), (12, 6)],
        ..Default::default()
    };
    let out = save::apply_live(&base, &live, 1);
    assert_eq!(out.header.version, 0x60);
    let Stats::Bits(e) = &out.body.as_ref().unwrap().stats else {
        panic!("bit-field stats")
    };
    let got: Vec<(u16, u16, i32)> = e.iter().map(|e| (e.id, e.layer, e.value)).collect();
    assert_eq!(got, [(0, 0, 30), (3, 0, 22), (12, 0, 6)]);
    let bytes = d2s::write(&out, &Tables).unwrap();
    let opts = ReadOptions {
        expansion: true,
        game: None,
    };
    let back = d2s::read(&bytes, &opts, &Tables).unwrap();
    assert_eq!(back.header.version, 0x60);
    // M08: a 0x60 base is left as it is.
    let same = save::apply_live(&synthetic_save(), &live, 1);
    assert_eq!(same.header.version, 0x60);
}

/// The store refuses to overwrite an old file whose item records would
/// pass through as loaded (old bit layout under a 0x60 header).
// Covers: specs/formats/d2s.md §1 r6
#[test]
fn old_item_records_that_pass_through_are_named() {
    let mut base = synthetic_save();
    let item = d2s::ItemEntry {
        bytes: vec![0x4A, 0x4D, 0, 0],
    };
    base.body.as_mut().unwrap().items = vec![item.clone()];
    // No inventory model: the loaded list would pass through.
    assert!(save::old_items_pass_through(&base, &save::Live::default()));
    // The running game rewrote the list.
    let mut live = save::Live::default();
    live.extra.items = Some(vec![item]);
    assert!(!save::old_items_pass_through(&base, &live));
    // No item at all: nothing passes through.
    assert!(!save::old_items_pass_through(
        &synthetic_save(),
        &save::Live::default()
    ));
}
