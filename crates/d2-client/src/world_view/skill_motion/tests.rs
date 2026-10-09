// Spec: specs/render/unit-composite.md §8; specs/skills/sequences.md §3, §4
//! Expected values from the REC-275 recordings (Wine, 1.14d, a level-30
//! barbarian in the Cold Plains): `facts/client/anim/a1-cold-plains-leap-bar.tsv`
//! and `facts/client/anim/a1-cold-plains-whirlwind-bar.tsv`.

use super::*;

const LEAP_FACTS: &str =
    include_str!("../../../../../facts/client/anim/a1-cold-plains-leap-bar.tsv");
const WHIRL_FACTS: &str =
    include_str!("../../../../../facts/client/anim/a1-cold-plains-whirlwind-bar.tsv");

/// The barbarian's charstats speeds (the leap's `s` 9 and the whirl's
/// step 0x6000 in the facts).
const BARBARIAN: Speeds = Speeds { walk: 6, run: 9 };

/// `skills.txt` `seqnum` of Leap and Whirlwind (`sequences.md` §4).
const SEQ_LEAP: u8 = 13;
const SEQ_WHIRL: u8 = 10;

/// `plrmode` rows of the drawn modes.
const MODE_A1: u32 = 7;
const MODE_S1: u32 = 13;

/// One `upd` row of a facts file: mode `m`, drawn frame `f >> 8`, the
/// motion record (`mfl` 2 = live: ticks left `mn`, `oz`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Upd {
    mode: u32,
    frame: usize,
    record: Option<(i32, i32)>,
}

fn upd_rows(facts: &str) -> Vec<Upd> {
    let mut head: Vec<&str> = Vec::new();
    let mut out = Vec::new();
    for line in facts.lines().filter(|l| !l.starts_with('#')) {
        let cols: Vec<&str> = line.split('\t').collect();
        if cols[0] == "k" {
            head = cols;
            continue;
        }
        if cols[0] != "upd" {
            continue;
        }
        let col = |name: &str| cols[head.iter().position(|h| *h == name).unwrap()];
        let num = |name: &str| col(name).parse::<i32>().unwrap();
        out.push(Upd {
            mode: num("m") as u32,
            frame: (num("f") >> 8) as usize,
            record: (col("mfl") == "2").then(|| (num("mn"), num("oz"))),
        });
    }
    out
}

/// The mode-18 stretches of a facts file and the row after each.
fn mode18_runs(facts: &str) -> Vec<Vec<Upd>> {
    let rows = upd_rows(facts);
    let mut runs: Vec<Vec<Upd>> = Vec::new();
    let mut prev = 0;
    for r in rows {
        if r.mode == 18 {
            if prev != 18 {
                runs.push(Vec::new());
            }
            runs.last_mut().unwrap().push(r);
        }
        prev = r.mode;
    }
    runs
}

/// Plays `run` from the request's update; each item is what the update
/// draws: (drawn frame, (ticks left, oz) of a live record).
/// What one update draws: (drawn mode, drawn frame, record).
type Drawn = (u32, usize, Option<(i32, i32)>);

fn play(mut run: SeqRun) -> Vec<Drawn> {
    let mut out = Vec::new();
    let mut live = true;
    while live && out.len() < 200 {
        let (m, f) = run.drawn();
        out.push((m, f, run.record().map(|r| (r.ticks_left, r.offset[2]))));
        live = run.update((0, 0), Some(BARBARIAN));
    }
    out
}

// Covers: specs/render/unit-composite.md §8 r1, §8 r2, §8 r6
#[test]
fn gravity_follows_the_two_branches_and_the_floor() {
    assert_eq!(leap_gravity(10), 5762);
    assert_eq!(leap_gravity(26), 8462 - 270 * 26);
    assert_eq!(leap_gravity(27), 500);
    assert_eq!(leap_gravity(100), 500);
}

// Covers: specs/render/unit-composite.md §8 r1, §8 r2, §8 r6
#[test]
fn timed_arc_starts_at_the_height_and_lands_after_n_updates() {
    let mut r = MotionRecord::default();
    timed_arc(&mut r, 0, 4);
    assert_eq!(r.acc[2], -0x1000);
    assert_eq!(r.ticks_left, 4);
    // vz = -az·n²/2 / n = 0x1000·16/2/4.
    assert_eq!(r.vel[2], 0x1000 * 4 / 2);
    let mut rec = r;
    let mut oz = Vec::new();
    for _ in 0..6 {
        rec.update(false, None).unwrap();
        oz.push(rec.offset[2]);
    }
    assert_eq!(rec.flags & motion::DONE, motion::DONE);
    assert!(oz.iter().any(|&z| z < 0), "the unit rises: {oz:?}");
}

// Covers: specs/render/unit-composite.md §8 r1, §8 r2, §8 r6
/// The recorded leaps (REC-275 recording note, `a1-cold-plains-leap-bar.tsv`):
/// s 9, d 8 → g 6302, n 13, vz 40963; d 11 → g 5492, n 18, vz 49428.
#[test]
fn leap_record_matches_the_recorded_n_g_and_vz() {
    assert_eq!(leap_record(10, 0), None);
    for (d, n, g, vz) in [(8, 13, 6302, 40963), (11, 18, 5492, 49428)] {
        let r = leap_record(d, 9).unwrap();
        assert_eq!(r.ticks_left, n, "d {d}");
        assert_eq!(r.acc[2], -g, "d {d}");
        assert_eq!(r.vel[2], vz, "d {d}");
    }
}

// Covers: specs/render/unit-composite.md §8 r1, §8 r2, §8 r6
/// `a1-cold-plains-leap-bar.tsv`: the record's ticks left and `oz` count
/// down one per update (13 → 0), done the update after 0.
#[test]
fn the_leap_record_steps_as_recorded() {
    let runs = mode18_runs(LEAP_FACTS);
    for (d, run) in [(8, &runs[0]), (11, &runs[1])] {
        let want: Vec<(i32, i32)> = run.iter().filter_map(|u| u.record).collect();
        let mut rec = leap_record(d, 9).unwrap();
        let mut got = vec![(rec.ticks_left, rec.offset[2])];
        loop {
            rec.update(false, None).unwrap();
            if rec.flags & motion::DONE != 0 {
                break;
            }
            got.push((rec.ticks_left, rec.offset[2]));
        }
        assert_eq!(got, want, "d {d}");
    }
}

// Covers: specs/skills/sequences.md §3, §4; specs/render/unit-composite.md §8 r1
/// `a1-cold-plains-leap-bar.tsv`, both leaps: frames 0–5 one per update,
/// the record from the update after frame 5's (event byte 1), frame 11
/// held while airborne, 12–14 after the landing, then the mode ends.
#[test]
fn the_leap_sequence_starts_the_arc_at_frame_five_and_holds_frame_eleven() {
    let frames = sequences::lookup(SEQ_LEAP, PREVIEW_CLASS).unwrap();
    let runs = mode18_runs(LEAP_FACTS);
    assert_eq!(runs.len(), 2);
    for (d, run) in [(8, &runs[0]), (11, &runs[1])] {
        let want: Vec<_> = run.iter().map(|u| (MODE_S1, u.frame, u.record)).collect();
        let got = play(SeqRun::leap(frames, (d, 0)));
        assert_eq!(got, want, "d {d}");
    }
}

// Covers: specs/skills/sequences.md §3, §4
/// `a1-cold-plains-whirlwind-bar.tsv`: A1 0, 1, 2, 3, 3, 4, 5, 6 then A1
/// 3, 4, 5, 6 one per update, for 24 updates (8 sub-tiles) and 40 (1 by
/// 14 sub-tiles: `0x006417F0` 14), then the mode ends.
#[test]
fn the_whirl_loops_its_last_four_frames_while_its_path_runs() {
    let frames = sequences::lookup(SEQ_WHIRL, PREVIEW_CLASS).unwrap();
    let runs = mode18_runs(WHIRL_FACTS);
    assert_eq!(runs.len(), 2);
    assert_eq!((runs[0].len(), runs[1].len()), (24, 40));
    for (to, run) in [((8, 0), &runs[0]), ((1, 14), &runs[1])] {
        let want: Vec<_> = run.iter().map(|u| (MODE_A1, u.frame, None)).collect();
        let got = play(SeqRun::whirl(frames, to));
        assert_eq!(got, want, "to {to:?}");
    }
    // A class without a Whirlwind sequence (bow) gets no run.
    assert_eq!(sequences::lookup(SEQ_WHIRL, 1), None);
}

// Covers: specs/skills/sequences.md §3
/// The whirl's mode-18 updates after its do (`a1-cold-plains-whirlwind-bar.tsv`:
/// 21 and 37 at step 0x6000).
#[test]
fn whirl_updates_count_the_full_steps_before_the_end() {
    assert_eq!(whirl_updates(8, 6), 21);
    assert_eq!(whirl_updates(14, 6), 37);
    assert_eq!(whirl_updates(0, 6), 1);
    assert_eq!(whirl_updates(8, 0), 1);
}

// Covers: specs/skills/sequences.md §3, §4
/// The resource: a Leap request starts mode 18 at frame 0, the arc's
/// offsets appear from the update after frame 5, and the run ends.
#[test]
fn skill_motion_runs_a_requested_leap() {
    use crate::bridge::world::ClientUnit;
    let key = UnitKey::new(0, 1);
    let mut world = ClientWorld::default();
    let mut u = ClientUnit::new(key);
    u.position = Some((10, 10));
    world.units.insert(key, u);
    world.local_player = Some(key);
    let row = SkillRow {
        srvdofunc: LEAP,
        seqnum: SEQ_LEAP,
        ..SkillRow::default()
    };
    let rows = |id: u16| (id == 132).then_some(row);
    let mut m = SkillMotion::default();
    world.server_ticks = 100;
    m.frame(&world, rows, Some(BARBARIAN), None);
    assert_eq!(m.drawn(), None);
    world.units.get_mut(&key).unwrap().last_mode_request = Some(ModeRequest {
        code: 0x15,
        record: [132, 0, 18, 10, 0, 0, 0],
    });
    m.frame(&world, rows, Some(BARBARIAN), None);
    assert_eq!(m.drawn(), Some((key, MODE_S1, 0)));
    let mut drawn = Vec::new();
    let mut lifted = Vec::new();
    for _ in 0..40 {
        world.server_ticks += 1;
        m.frame(&world, rows, Some(BARBARIAN), None);
        drawn.push(m.drawn().map(|d| d.2));
        lifted.push(m.offsets().get(&key).map(|o| o.1));
    }
    assert_eq!(&drawn[..6], &[1, 2, 3, 4, 5, 6].map(Some));
    assert_eq!(lifted[4], None, "no record before frame 5");
    assert_eq!(lifted[5], Some(0), "made at frame 5, first drawn unstepped");
    assert_eq!(lifted[6], Some(-20));
    assert_eq!(drawn.last(), Some(&None), "the mode ended");
    assert!(m.offsets().is_empty());
}
