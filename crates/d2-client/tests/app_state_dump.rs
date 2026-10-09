// Spec: specs/tools/state-snapshot.md (§1, §3; test vector 5)
//! `d2-client state-dump` on the user's install: the headless run of the
//! play game writes header, one snapshot per server tick, footer; two
//! runs give the same bytes; the joined player is in the snapshots.

use d2_client::app::single_player::Character;
use d2_client::app::state_dump::{self, DumpArgs, DumpGame, RunInfo};
use d2_server::adapters::character::LoadContext;

mod app_support;

fn run(character: Character, ticks: u32, every: u32) -> Vec<u8> {
    run_with(character, ticks, every, &[])
}

fn run_with(character: Character, ticks: u32, every: u32, pokes: &[&str]) -> Vec<u8> {
    let args = DumpArgs {
        save: None,
        seed: Some(1234),
        difficulty: 0,
        ticks,
        every,
        out: "unused".into(),
        game_dir: None,
        date: Some("2026-10-09".into()),
        pokes: pokes
            .iter()
            .map(|p| d2_client::app::poke::parse_poke_arg(p).unwrap())
            .collect(),
        input: None,
        packets: None,
        rng: None,
    };
    let mut game = DumpGame::resolve(&args, app_support::game_data(), None).unwrap();
    game.character = character;
    let info = RunInfo {
        tool: "d2-client state-dump test".into(),
        date: "2026-10-09".into(),
        command: "test".into(),
        save: Some("ScnAma.d2s".into()),
    };
    let mut out = Vec::new();
    let r = state_dump::dump(game, ticks, every, &info, &mut out).unwrap();
    assert_eq!(r.ticks, ticks);
    out
}

fn scn_ama() -> Character {
    let save = d2_formats::d2s::D2s::new_stub(b"ScnAma", 0, 0x20, 0).unwrap();
    Character::Save(
        Box::new(save),
        LoadContext {
            difficulty: 0,
            map_seed_applies: false,
        },
    )
}

// Covers: specs/tools/state-snapshot.md §1 r1, §1 r2, §1 r3, §3 r1, §3 r2
#[test]
#[ignore = "real data: needs D2_GAME_DIR (tools/realdata-gate.sh)"]
fn a_dump_is_one_snapshot_per_tick_and_repeats_byte_for_byte() {
    let a = run(scn_ama(), 6, 1);
    let b = run(scn_ama(), 6, 1);
    assert_eq!(a, b, "two runs differ");
    let text = String::from_utf8(a).unwrap();
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 6 + 2);
    assert!(lines[0].starts_with(r#"{"k":"header","format":"state-1","side":"d2rs""#));
    assert!(lines[0].contains(r#""seed":1234}"#), "{}", lines[0]);
    assert!(lines[7].starts_with(r#"{"k":"footer","snaps":6,"#));
    // Frames 1, 2, … in order (§3: the state after tick N is frame N).
    for (i, l) in lines[1..7].iter().enumerate() {
        let want = format!(r#"{{"k":"snap","f":{},"seed":["#, i + 1);
        assert!(l.starts_with(&want), "{l}");
    }
    // The joined player (type 0) is in the last snapshot, with a position.
    assert!(lines[6].contains(r#"{"ut":0,"g":"#), "{}", lines[6]);
}

// Covers: specs/tools/state-snapshot.md §3 r3
#[test]
#[ignore = "real data: needs D2_GAME_DIR (tools/realdata-gate.sh)"]
fn every_n_keeps_the_multiples_of_n() {
    let text = String::from_utf8(run(Character::New, 6, 3)).unwrap();
    let frames: Vec<&str> = text
        .lines()
        .filter(|l| l.starts_with(r#"{"k":"snap""#))
        .map(|l| l.split(',').nth(1).unwrap())
        .collect();
    assert_eq!(frames, [r#""f":3"#, r#""f":6"#]);
}
// Covers: specs/tools/poke.md §2 r6
// (state-dump --poke: after frame f − 1's snapshot, before frame f)
#[test]
#[ignore = "real data: needs D2_GAME_DIR (tools/realdata-gate.sh)"]
fn a_poke_spawn_runs_before_its_frame_and_the_unit_is_in_that_snapshot() {
    let pokes = [
        "4 spawn 19 @x+3 @y+3 normal",
        "4 seed-unit @1:19 0x12345678 666",
    ];
    let a = run_with(scn_ama(), 8, 1, &pokes);
    assert_eq!(a, run_with(scn_ama(), 8, 1, &pokes), "two runs differ");
    let text = String::from_utf8(a).unwrap();
    let lines: Vec<&str> = text.lines().collect();
    // header, snaps 1..3, two poke lines, snaps 4..8, footer
    assert_eq!(lines.len(), 1 + 8 + 2 + 1, "{text}");
    assert!(
        lines[3].starts_with(r#"{"k":"snap","f":3,"#),
        "{}",
        lines[3]
    );
    let spawn = lines[4];
    assert!(
        spawn.starts_with(r#"{"k":"poke","f":4,"frame":3,"i":0,"d":"spawn","r":"ok","guid":"#),
        "{spawn}"
    );
    assert!(
        lines[5]
            .starts_with(r#"{"k":"poke","f":4,"frame":3,"i":1,"d":"seed-unit","r":"ok","src":"#),
        "{}",
        lines[5]
    );
    let guid: u32 = spawn
        .split(r#""guid":"#)
        .nth(1)
        .unwrap()
        .split(',')
        .next()
        .unwrap()
        .parse()
        .unwrap();
    let unit = format!(r#"{{"ut":1,"g":{guid},"cl":19,"#);
    // Not in frame 3's snapshot; in frame 4's, with the poked seed (the
    // spawn's own tick advanced it from there).
    assert!(!lines[3].contains(&unit), "{}", lines[3]);
    assert!(
        lines[6].starts_with(r#"{"k":"snap","f":4,"#),
        "{}",
        lines[6]
    );
    assert!(lines[6].contains(&unit), "{}", lines[6]);
}

/// (frame, value) at each change of `field` of the first unit that
/// `pick` accepts, over a dump's snapshots.
fn changes(dump: &[u8], pick: impl Fn(&serde_json::Value) -> bool, field: &str) -> Vec<(u64, i64)> {
    let mut out: Vec<(u64, i64)> = Vec::new();
    for line in std::str::from_utf8(dump).unwrap().lines() {
        let v: serde_json::Value = serde_json::from_str(line).unwrap();
        if v["k"] != "snap" {
            continue;
        }
        let f = v["f"].as_u64().unwrap();
        let Some(u) = v["units"].as_array().unwrap().iter().find(|u| pick(u)) else {
            continue;
        };
        let x = u[field].as_i64().unwrap();
        if out.last().is_none_or(|&(_, h)| h != x) {
            out.push((f, x));
        }
    }
    out
}

/// The check's pokes (`traces/checks/combat-fallen-hits-player.check`).
const FALLEN_POKES: [&str; 4] = [
    "4 warp 2",
    "30 seed-game 0x00001234 666",
    "30 seed-unit @player 0x00000055 666",
    "30 spawn 19 @x+2 @y normal",
];

// Covers: specs/skills/bodies-2.md §2.1; specs/monsters/umod-callbacks.md §2 r1; specs/monsters/init.md §6 r12
// (traces/checks/combat-fallen-hits-player.check, PC1-B 2026-10-09)
#[test]
#[ignore = "real data: needs D2_GAME_DIR (tools/realdata-gate.sh)"]
fn a_fallen_party_hits_the_player_on_the_recorded_frames() {
    // 1.14d (PC1-B): the player's life 12800 → 12321 at frame 77, then
    // 11866 at 98, 11397 at 124, 11039 at 137, 10560 at 173. Before the
    // mode damage of the mode set the monsters' to-hit was 0 and the
    // player never lost life.
    let dump = run_with(scn_ama(), 200, 1, &FALLEN_POKES);
    let life = changes(&dump, |u| u["ut"] == 0, "hp");
    // The player joins at 12800 (frame 2) and loses life only on the hits.
    assert_eq!(life.first().map(|c| c.1), Some(12800), "{life:?}");
    assert_eq!(
        life[1..],
        [
            (77, 12321),
            (98, 11866),
            (124, 11397),
            (137, 11039),
            (173, 10560)
        ],
        "{life:?}"
    );
}

// Covers: specs/monsters/ai-bodies.md §9.4 r5; specs/monsters/population.md §10.2 r1, §10.2 r3
// (traces/checks/combat-fallen-hits-player.check, PC1-B 2026-10-09)
#[test]
#[ignore = "real data: needs D2_GAME_DIR (tools/realdata-gate.sh)"]
fn the_fallen_leader_shouts_on_the_recorded_frames() {
    // The party leader (the poke's first GUID, own pack leader through
    // the owner data `0x0058F030`): 1.14d modes from frame 41: S2 (9),
    // NU 65, S2 90, NU 114, A2 129. Without the owner link it took A2 at
    // 41.
    let dump = run_with(scn_ama(), 130, 1, &FALLEN_POKES);
    let text = std::str::from_utf8(&dump).unwrap();
    let guid: u64 = text
        .lines()
        .find(|l| l.contains(r#""d":"spawn""#))
        .and_then(|l| l.split(r#""guid":"#).nth(1))
        .and_then(|r| r.split(',').next())
        .unwrap()
        .parse()
        .unwrap();
    let modes = changes(&dump, |u| u["ut"] == 1 && u["g"] == guid, "m");
    assert_eq!(
        modes[1..],
        [(41, 9), (65, 1), (90, 9), (114, 1), (129, 5)],
        "{modes:?}"
    );
}
