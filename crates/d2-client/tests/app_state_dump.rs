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

/// The `x` (or another number field) of the player object of a snap line.
fn player_field(snap: &str, field: &str) -> i64 {
    let p = snap.split(r#"{"ut":0,"#).nth(1).expect("a player");
    let key = format!(r#""{field}":"#);
    p.split(&key).nth(1).expect(field)[..]
        .split([',', '}'])
        .next()
        .unwrap()
        .parse()
        .unwrap()
}

// Covers: specs/tools/poke.md §5 r3
// (state-dump --poke "<f> msg ...": the bytes go through the local
// client's sender, duplicate filter included, and frame f's drain
// handles them)
#[test]
#[ignore = "real data: needs D2_GAME_DIR (tools/realdata-gate.sh)"]
fn a_poke_msg_walk_reaches_the_server_and_the_duplicate_is_filtered() {
    let pokes = ["4 msg 0x01 @x+5 @y", "4 msg 0x01 @x+5 @y"];
    let a = run_with(scn_ama(), 12, 1, &pokes);
    assert_eq!(a, run_with(scn_ama(), 12, 1, &pokes), "two runs differ");
    let text = String::from_utf8(a).unwrap();
    let lines: Vec<&str> = text.lines().collect();
    // header, snaps 1..3, two poke lines, snaps 4..12, footer
    assert_eq!(lines.len(), 1 + 12 + 2 + 1, "{text}");
    assert!(
        lines[4].starts_with(
            r#"{"k":"poke","f":4,"frame":3,"i":0,"d":"msg","r":"ok","src":"msg 1 @x+5 @y"}"#
        ),
        "{}",
        lines[4]
    );
    assert_eq!(
        lines[5],
        r#"{"k":"poke","f":4,"frame":3,"i":1,"d":"msg","r":"failed","note":"duplicate filter","src":"msg 1 @x+5 @y"}"#
    );
    // Frame 4's drain handles the walk; by frame 12 the player has
    // walked east.
    let x3 = player_field(lines[3], "x");
    assert!(player_field(lines[14], "x") > x3, "{}", lines[14]);
}
