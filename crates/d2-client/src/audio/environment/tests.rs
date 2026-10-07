// Spec: specs/audio/environment.md
//! Test vectors of `audio/environment.md` against a recording fake of the
//! sound layer (call log + scripted rolls).

use std::collections::{BTreeMap, VecDeque};

use super::*;
use crate::audio::calls::FLAG_NO_FADE_IN;

#[derive(Clone, Debug, PartialEq, Eq)]
enum Call {
    Request {
        id: i32,
        unit: Option<UnitKey>,
        flags: u32,
        offset: u32,
        h: Handle,
    },
    SetVolume(Handle, i32),
    Fade(Handle, i32, u32, u32),
    StopHandle(Handle),
    StopId(i32),
    StopSongs,
    Stop52(i32, i32),
    Stop72(i32, i32),
    SetPosition(Handle, i32, i32, i32),
    Roll(i32, u32),
}

#[derive(Clone, Debug)]
struct Req {
    h: Handle,
    id: i32,
    vol: i32,
}

#[derive(Default)]
struct Fake {
    t: u32,
    calls: Vec<Call>,
    rolls: VecDeque<u32>,
    next: Handle,
    reqs: Vec<Req>,
    positions: BTreeMap<i32, u32>,
    blocks: BTreeMap<i32, [i32; 3]>,
    speech: bool,
    /// Ids whose request returns 0.
    refuse: Vec<i32>,
}

impl Fake {
    fn take(&mut self) -> Vec<Call> {
        std::mem::take(&mut self.calls)
    }

    /// Requests, song stops and id stops only.
    fn music(&mut self) -> Vec<Call> {
        self.take()
            .into_iter()
            .filter(|c| matches!(c, Call::Request { .. } | Call::StopSongs | Call::StopId(_)))
            .collect()
    }
}

impl SoundCalls for Fake {
    fn request(
        &mut self,
        id: i32,
        unit: Option<UnitKey>,
        delay: u32,
        flags: u32,
        offset: u32,
    ) -> Handle {
        assert_eq!(delay, 0);
        let h = if id < 1 || self.refuse.contains(&id) {
            0
        } else {
            self.next += 1;
            self.reqs.push(Req {
                h: self.next,
                id,
                vol: 255,
            });
            self.next
        };
        self.calls.push(Call::Request {
            id,
            unit,
            flags,
            offset,
            h,
        });
        h
    }
    fn set_volume(&mut self, h: Handle, v: i32) {
        if let Some(r) = self.reqs.iter_mut().find(|r| r.h == h) {
            r.vol = v;
        }
        self.calls.push(Call::SetVolume(h, v));
    }
    fn fade(&mut self, h: Handle, target: i32, delay: u32, len: u32) {
        self.calls.push(Call::Fade(h, target, delay, len));
    }
    fn detach(&mut self, _: Handle, _: UnitKey, _: bool) {
        unreachable!("not used by the environment machines");
    }
    fn stop_handle(&mut self, h: Handle) {
        self.calls.push(Call::StopHandle(h));
    }
    fn stop_id(&mut self, id: i32) {
        self.calls.push(Call::StopId(id));
    }
    fn stop_songs(&mut self) {
        self.calls.push(Call::StopSongs);
    }
    fn stop_52_71_except(&mut self, a: i32, b: i32) {
        self.calls.push(Call::Stop52(a, b));
    }
    fn stop_72_201_except(&mut self, a: i32, b: i32) {
        self.calls.push(Call::Stop72(a, b));
    }
    fn stop_speech(&mut self) {
        unreachable!("not used by the environment machines");
    }
    fn speaking(&self, _: UnitKey) -> bool {
        unreachable!("not used by the environment machines");
    }
    fn any_speech(&self) -> bool {
        self.speech
    }
    fn is_active(&self, h: Handle) -> bool {
        self.reqs.iter().any(|r| r.h == h)
    }
    fn sound_tick(&self) -> u32 {
        self.t
    }
    fn roll(&mut self, n: i32) -> u32 {
        let r = self.rolls.pop_front().unwrap_or(0);
        assert!(n > 0 && r < n as u32, "roll({n}) scripted {r}");
        self.calls.push(Call::Roll(n, r));
        r
    }
}

impl EnvCalls for Fake {
    fn id_requested(&self, id: i32) -> bool {
        self.reqs.iter().any(|r| r.id == id)
    }
    fn music_active(&self) -> bool {
        self.reqs.iter().any(|r| (4657..=4698).contains(&r.id))
    }
    fn play_position(&self, id: i32) -> Option<u32> {
        self.positions.get(&id).copied()
    }
    fn blocks(&self, id: i32) -> [i32; 3] {
        self.blocks.get(&id).copied().unwrap_or([-1, -1, -1])
    }
    fn volume(&self, h: Handle) -> Option<i32> {
        self.reqs.iter().find(|r| r.h == h).map(|r| r.vol)
    }
    fn set_position(&mut self, h: Handle, x: i32, y: i32, z: i32) {
        self.calls.push(Call::SetPosition(h, x, y, z));
    }
}

#[derive(Default)]
struct Hooks {
    quest_ok: bool,
    events: Vec<u8>,
}

impl EnvHooks for Hooks {
    fn quest_check(&self, _: u8) -> bool {
        self.quest_ok
    }
    fn player_event(&mut self, _: &mut dyn SoundCalls, e: u8) {
        self.events.push(e);
    }
}

const P: UnitKey = UnitKey::new(0, 1);
const SONG_A: i32 = 4660;
const SONG_B: i32 = 4670;
/// `music_caves` blocks (§2 r8 test vectors).
const CAVES_BLOCKS: [i32; 3] = [1_478_063, -1, -1];
/// `music_wilderness` blocks.
const WILD_BLOCKS: [i32; 3] = [3_457_024, 6_755_328, -1];

fn song_row(song: i32) -> EnvRow {
    EnvRow {
        song,
        ..EnvRow::default()
    }
}

fn amb_row() -> EnvRow {
    EnvRow {
        song: 0,
        day_ambience: 52,
        night_ambience: 53,
        day_event: 72,
        night_event: 80,
        event_delay: 250,
        indoors: 0,
        material1: 2,
        material2: 3,
    }
}

fn input(level: u32, env: Option<EnvRow>) -> TickInput {
    TickInput {
        level,
        env,
        day_phase: 1,
        weather_active: false,
        rain_level: 0,
        master_volume: 100,
        music_volume: 50,
        c: 0,
        player: Some(PlayerState {
            key: P,
            alive: true,
            last_voice: 0,
        }),
        level_count: 137,
    }
}

fn env() -> Environment {
    Environment::new(SongRange::LIVE)
}

fn req(id: i32, unit: Option<UnitKey>, flags: u32, offset: u32, h: Handle) -> Call {
    Call::Request {
        id,
        unit,
        flags,
        offset,
        h,
    }
}

// Covers: specs/audio/environment.md §1 r1
#[test]
fn no_level_does_nothing() {
    let (mut e, mut s, mut h) = (env(), Fake::default(), Hooks::default());
    s.t = 500;
    let mut inp = input(0, Some(amb_row()));
    inp.weather_active = true;
    assert_eq!(e.tick(&mut s, &mut h, &inp), None);
    assert!(s.calls.is_empty());
    assert_eq!(e, env());
}

// Covers: specs/audio/environment.md §1 r2, §1 r4
#[test]
fn env_row_out_of_range_is_no_row() {
    let rows = vec![song_row(4657); 50];
    assert_eq!(env_row(&rows, 49), Some(song_row(4657)));
    assert_eq!(env_row(&rows, 50), None);
    // No row: song 0, ambience 0, no event; the music stops all songs.
    let (mut e, mut s, mut h) = (env(), Fake::default(), Hooks::default());
    e.tick(&mut s, &mut h, &input(1, None));
    assert_eq!(e.music.cur, 0);
    assert_eq!(e.ambience.bed, 0);
    assert_eq!(s.music(), vec![Call::StopSongs]);
    // The song range of the live rows (sound-table §1 r5).
    let rows = [song_row(0), song_row(4684), song_row(4657), song_row(-1)];
    assert_eq!(SongRange::from_rows(&rows), Some(SongRange::LIVE));
    assert_eq!(SongRange::from_rows(&[song_row(0)]), None);
}

// Covers: specs/audio/environment.md §1 r3
#[test]
fn day_is_phase_1_to_3() {
    let day: Vec<u32> = (0..8).filter(|&p| is_day(p)).collect();
    assert_eq!(day, vec![1, 2, 3]);
}

// Covers: specs/audio/environment.md §2 r1, §2 r3, §2 r6
#[test]
fn first_song_starts_at_once_without_fade() {
    let (mut e, mut s, mut h) = (env(), Fake::default(), Hooks::default());
    s.t = 10;
    e.tick(&mut s, &mut h, &input(1, Some(song_row(SONG_A))));
    assert_eq!(e.music.cur, SONG_A);
    assert_eq!(e.music.ts, 10);
    assert_eq!(
        s.music(),
        vec![Call::StopSongs, req(SONG_A, None, FLAG_NO_FADE_IN, 0, 1)]
    );
    // Next tick: the request exists, nothing more.
    s.t = 11;
    e.tick(&mut s, &mut h, &input(1, Some(song_row(SONG_A))));
    assert!(s.music().is_empty());
    // A `Song` outside the range is song 0 (stinger ids included).
    let (mut e, mut s) = (env(), Fake::default());
    e.tick(&mut s, &mut h, &input(1, Some(song_row(4685))));
    assert_eq!(e.music.cur, 0);
    assert_eq!(s.music(), vec![Call::StopSongs]);
}

// Covers: specs/audio/environment.md §2 r2, §2 r3, §2 r6
#[test]
fn level_change_waits_75_ticks() {
    let (mut e, mut s, mut h) = (env(), Fake::default(), Hooks::default());
    e.tick(&mut s, &mut h, &input(1, Some(song_row(SONG_A))));
    e.music.set_resume(SONG_B, 1234);
    s.take();
    for t in 100..=174 {
        s.t = t;
        e.tick(&mut s, &mut h, &input(2, Some(song_row(SONG_B))));
        assert_eq!(e.music.cur, SONG_A, "T {t}");
        assert!(s.music().is_empty(), "T {t}");
    }
    assert_eq!(e.music.tl, 100);
    assert_eq!(e.music.last_level, 2);
    s.t = 175;
    e.tick(&mut s, &mut h, &input(2, Some(song_row(SONG_B))));
    assert_eq!(e.music.cur, SONG_B);
    assert_eq!(e.music.ts, 175);
    // A still active → flags 0 (fade in); A gets the stop, B its offset.
    assert_eq!(
        s.music(),
        vec![Call::StopSongs, req(SONG_B, None, 0, 1234, 2)]
    );
}

// Covers: specs/audio/environment.md §2 r4, §2 r5, §3 r7, §edge-cases-original-bugs r2
#[test]
fn music_volume_0_stops_songs_but_not_stingers() {
    let (mut e, mut s, mut h) = (env(), Fake::default(), Hooks::default());
    let mut inp = input(1, Some(song_row(SONG_A)));
    inp.music_volume = 0;
    e.tick(&mut s, &mut h, &inp);
    assert_eq!(e.music.cur, SONG_A);
    assert_eq!(s.music(), vec![Call::StopSongs]);
    let st = stinger_for_event(33, 2961).unwrap();
    inp.c = 500;
    e.music.start_stinger(&mut s, inp.c, st);
    assert_eq!(s.music(), vec![Call::StopSongs]);
    s.t = 1;
    e.tick(&mut s, &mut h, &inp);
    // Song stop (r4), then M at tM = C + 0 (r5 → §3 r4); no song request.
    assert_eq!(s.music(), vec![Call::StopSongs, req(4685, None, 0, 0, 1)]);
    // At the hold end (C 975) the line (due at C 925) is requested on P
    // and the stinger ends; not audible → no stop of M.
    inp.c = 975;
    s.t = 2;
    e.tick(&mut s, &mut h, &inp);
    assert!(!e.music.stinger.active);
    assert_eq!(
        s.music(),
        vec![Call::StopSongs, req(2961, Some(P), 0, 0, 2)]
    );
    // Master Volume 0 is not audible either.
    let (mut e, mut s) = (env(), Fake::default());
    let mut inp = input(1, Some(song_row(SONG_A)));
    inp.master_volume = 0;
    e.tick(&mut s, &mut h, &inp);
    assert_eq!(s.music(), vec![Call::StopSongs]);
}

// Covers: specs/audio/environment.md §2 r8, §edge-cases-original-bugs r5
#[test]
fn resume_points() {
    assert_eq!(resume_point(CAVES_BLOCKS, 1_000_000), 1_478_063);
    assert_eq!(resume_point(CAVES_BLOCKS, 2_000_000), 0);
    assert_eq!(resume_point(WILD_BLOCKS, 4_000_000), 6_755_328);
    assert_eq!(resume_point(WILD_BLOCKS, 0), 3_457_024);
    assert_eq!(resume_point(WILD_BLOCKS, 7_000_000), 0);
    assert_eq!(resume_point([-1, -1, -1], 5), 0);
    assert_eq!(resume_point([10, 20, 30], 25), 30);
}

// Covers: specs/audio/environment.md §2 r8
#[test]
fn resume_bookkeeping_every_125_ticks() {
    let (mut e, mut s, mut h) = (env(), Fake::default(), Hooks::default());
    s.blocks.insert(SONG_A, CAVES_BLOCKS);
    s.t = 0;
    e.tick(&mut s, &mut h, &input(1, Some(song_row(SONG_A))));
    s.positions.insert(SONG_A, 1_000_000);
    s.t = 125;
    e.tick(&mut s, &mut h, &input(1, Some(song_row(SONG_A))));
    assert_eq!(e.music.resume(SONG_A), 0, "T must exceed Ts + 125");
    s.t = 126;
    e.tick(&mut s, &mut h, &input(1, Some(song_row(SONG_A))));
    assert_eq!(e.music.resume(SONG_A), 1_478_063);
    assert_eq!(e.music.ts, 126);
    // Not playing: nothing.
    s.positions.clear();
    s.t = 300;
    e.tick(&mut s, &mut h, &input(1, Some(song_row(SONG_A))));
    assert_eq!(e.music.ts, 126);
}

// Covers: specs/audio/environment.md §2 r9
#[test]
fn reset_clears_music_and_stinger() {
    let (mut e, mut s, mut h) = (env(), Fake::default(), Hooks::default());
    s.t = 7;
    e.tick(&mut s, &mut h, &input(1, Some(song_row(SONG_A))));
    e.music.set_resume(SONG_A, 99);
    e.music
        .start_stinger(&mut s, 3, stinger_for_event(35, 2961).unwrap());
    e.music.reset();
    assert_eq!(e.music, Music::new(SongRange::LIVE));
}

// Covers: specs/audio/environment.md §3 r3, §3 r4, §3 row3
#[test]
fn stinger_event_35_at_c_1000() {
    let (mut e, mut s, mut h) = (env(), Fake::default(), Hooks::default());
    let mut inp = input(1, Some(song_row(SONG_A)));
    e.tick(&mut s, &mut h, &inp);
    s.take();
    s.reqs.clear(); // the song ended: §2 r6 would request it again
    let st = stinger_for_event(35, 2961).unwrap();
    assert_eq!(st.s, 2963);
    e.music.start_stinger(&mut s, 1000, st);
    assert_eq!(
        e.music.stinger,
        Stinger {
            active: true,
            m: 4688,
            tm: 1000,
            m_pending: true,
            s: 2963,
            ts: 1250,
            s_pending: true,
            th: 1300,
        }
    );
    let mut log = Vec::new();
    for c in 1000..=1300 {
        inp.c = c;
        s.t = c;
        e.tick(&mut s, &mut h, &inp);
        for call in s.music() {
            log.push((c, call));
        }
    }
    assert_eq!(
        log,
        vec![
            // The stinger's own song stop (§3 r2), then M at tM (r4).
            (1000, Call::StopSongs),
            (1000, req(4688, None, 0, 0, 2)),
            (1250, req(2963, Some(P), 0, 0, 3)),
            // Hold over: the stinger track is stopped, the song requested.
            (1300, Call::StopId(4688)),
            (1300, Call::StopSongs),
            (1300, req(SONG_A, None, 0, 0, 4)),
        ]
    );
    assert!(!e.music.stinger.active);
}

// Covers: specs/audio/environment.md §3 r1, §3 r2
#[test]
fn stinger_stores_block_k_of_playing_song() {
    let (mut e, mut s, mut h) = (env(), Fake::default(), Hooks::default());
    s.blocks.insert(SONG_A, WILD_BLOCKS);
    e.tick(&mut s, &mut h, &input(1, Some(song_row(SONG_A))));
    s.take();
    s.positions.insert(SONG_A, 5);
    s.t = 40;
    e.music
        .start_stinger(&mut s, 0, stinger_for_event(34, 2961).unwrap());
    assert_eq!(e.music.resume(SONG_A), 3_457_024);
    assert_eq!(e.music.ts, 40);
    assert_eq!(s.take(), vec![Call::StopSongs]);
    // k = 0 → 0.
    e.music
        .start_stinger(&mut s, 0, stinger_for_event(33, 2961).unwrap());
    assert_eq!(e.music.resume(SONG_A), 0);
    // Not playing: resume and Ts kept.
    e.music.set_resume(SONG_A, 77);
    s.positions.clear();
    s.t = 90;
    e.music
        .start_stinger(&mut s, 0, stinger_for_event(34, 2961).unwrap());
    assert_eq!(e.music.resume(SONG_A), 77);
    assert_eq!(e.music.ts, 40);
}

// Covers: specs/audio/environment.md §3 r4
#[test]
fn stinger_line_needs_living_player() {
    let (mut e, mut s, mut h) = (env(), Fake::default(), Hooks::default());
    let mut inp = input(1, Some(song_row(SONG_A)));
    inp.player = Some(PlayerState {
        key: P,
        alive: false,
        last_voice: 0,
    });
    e.music
        .start_stinger(&mut s, 0, stinger_for_event(35, 2961).unwrap());
    s.take();
    inp.c = 250;
    e.tick(&mut s, &mut h, &inp);
    assert_eq!(s.music(), vec![req(4688, None, 0, 0, 1)]);
    assert!(!e.music.stinger.s_pending);
}

// Covers: specs/audio/environment.md §3 r5
#[test]
fn stinger_rearm() {
    let mut m = Music::new(SongRange::LIVE);
    m.rearm_stinger(100, 25, true);
    assert_eq!(m.stinger, Stinger::default(), "only while active");
    let mut s = Fake::default();
    m.start_stinger(&mut s, 1000, stinger_for_event(35, 2961).unwrap());
    m.rearm_stinger(1100, 25, true);
    assert_eq!(
        (m.stinger.ts, m.stinger.s_pending, m.stinger.th),
        (1125, true, 1175)
    );
    m.rearm_stinger(1100, 25, false);
    assert_eq!(
        (m.stinger.ts, m.stinger.s_pending, m.stinger.th),
        (1125, false, 1175)
    );
}

// Covers: specs/audio/environment.md §3 r6, §3 row1, §3 row2, §3 row3, §3 row4, §3 row5, §3 row6, §3 row7, §3 row8, §3 row9, §3 row10, §3 row11, §edge-cases-original-bugs r3
#[test]
fn stinger_table() {
    // (event, M, dM, H, k, dS, play) as in §3 r6.
    let rows: [(u8, i32, u32, u32, u8, u32, bool); 11] = [
        (33, 4685, 0, 475, 0, 425, true),
        (34, 4686, 475, 800, 1, 525, true),
        (35, 4688, 0, 300, 1, 250, true),
        (37, 0, 0, 1500, 0, 0, false),
        (50, 4693, 475, 1100, 1, 525, true),
        (52, 4694, 0, 425, 1, 375, true),
        (66, 4692, 0, 425, 0, 375, true),
        (75, 4695, 0, 475, 0, 325, true),
        (80, 4698, 0, 500, 1, 475, true),
        (82, 4697, 0, 550, 1, 400, true),
        (83, 4696, 0, 525, 1, 425, true),
    ];
    for (e, m, dm, h, k, ds, play) in rows {
        let k = if k == 1 {
            ResumeBlock::Block(BlockIndex::B1)
        } else {
            ResumeBlock::Start
        };
        assert_eq!(
            stinger_for_event(e, 3273),
            Some(StingerArgs {
                m,
                dm,
                h,
                k,
                s: 3273 + i32::from(e) - 33,
                ds,
                play
            }),
            "event {e}"
        );
    }
    for e in (0..=255u8).filter(|e| !rows.iter().any(|r| r.0 == *e)) {
        assert_eq!(stinger_for_event(e, 3273), None);
    }
    let silent = |m, h| StingerArgs {
        m,
        dm: 0,
        h,
        k: ResumeBlock::Start,
        s: 0,
        ds: 0,
        play: false,
    };
    assert_eq!(STINGER_COMPELLING, silent(4687, 350));
    assert_eq!(STINGER_IZUAL, silent(4691, 250));

    // Event 37 silences music for 1,500 updates and never plays its line.
    let (mut e, mut s, mut h) = (env(), Fake::default(), Hooks::default());
    let mut inp = input(1, Some(song_row(SONG_A)));
    e.tick(&mut s, &mut h, &inp);
    s.reqs.clear();
    s.take();
    e.music
        .start_stinger(&mut s, 0, stinger_for_event(37, 2961).unwrap());
    let mut requested = Vec::new();
    for c in 0..1500 {
        inp.c = c;
        e.tick(&mut s, &mut h, &inp);
        requested.extend(
            s.music()
                .into_iter()
                .filter(|c| matches!(c, Call::Request { .. })),
        );
    }
    assert_eq!(requested, vec![req(0, None, 0, 0, 0)]);
    inp.c = 1500;
    e.tick(&mut s, &mut h, &inp);
    // No music request left active and resume 0 → no fade-in.
    assert!(s
        .music()
        .contains(&req(SONG_A, None, FLAG_NO_FADE_IN, 0, 2)));
}

/// Drives one tick at T = C = `t` on level `l` (no song, no ambience).
fn entry_tick(e: &mut Environment, s: &mut Fake, h: &mut Hooks, l: u32, t: u32, last_voice: u32) {
    let mut inp = input(l, Some(EnvRow::default()));
    inp.c = t;
    inp.player = Some(PlayerState {
        key: P,
        alive: true,
        last_voice,
    });
    s.t = t;
    e.tick(s, h, &inp);
}

// Covers: specs/audio/environment.md §2 r7, §4 r1, §4 r2
#[test]
fn entry_line_on_first_entry() {
    let (mut e, mut s, mut h) = (env(), Fake::default(), Hooks::default());
    h.quest_ok = true;
    entry_tick(&mut e, &mut s, &mut h, 1, 1000, 0);
    // Level 3 (`find_wilderness`, q 1) entered at T 1100: line at T − Tl = 62.
    for t in 1100..=1161 {
        entry_tick(&mut e, &mut s, &mut h, 3, t, 0);
    }
    assert!(h.events.is_empty());
    entry_tick(&mut e, &mut s, &mut h, 3, 1162, 0);
    assert_eq!(h.events, vec![47]);
    assert_eq!(e.music.last_announced, 3);
    // Every level of the record is flagged: entering 5 plays nothing.
    for l in [2, 3, 4, 5, 6, 7] {
        assert!(e.entry.is_flagged(l));
    }
    for t in 1200..1300 {
        entry_tick(&mut e, &mut s, &mut h, 5, t, 0);
    }
    assert_eq!(h.events, vec![47]);
}

// Covers: specs/audio/environment.md §4 r2, §4 r3, §edge-cases-original-bugs r4
#[test]
fn entry_line_skipped_is_lost() {
    // Hero spoke within 62 updates: flagged, no line, never retried.
    let (mut e, mut s, mut h) = (env(), Fake::default(), Hooks::default());
    h.quest_ok = true;
    for t in 1000..1100 {
        entry_tick(&mut e, &mut s, &mut h, 8, t, 1000);
    }
    assert!(h.events.is_empty());
    assert!(e.entry.is_flagged(8));
    entry_tick(&mut e, &mut s, &mut h, 1, 1200, 0);
    for t in 1300..1400 {
        entry_tick(&mut e, &mut s, &mut h, 8, t, 0);
    }
    assert!(h.events.is_empty());
    // C − P+0x7C = 63 passes.
    let (mut e, mut s) = (env(), Fake::default());
    for t in 1000..=1062 {
        entry_tick(&mut e, &mut s, &mut h, 8, t, 1062 - 62 - 1);
    }
    assert_eq!(h.events, vec![41]);
    // Quest check fails → nothing, flagged.
    let (mut e, mut s, mut h) = (env(), Fake::default(), Hooks::default());
    for t in 0..100 {
        entry_tick(&mut e, &mut s, &mut h, 74, t + 1000, 0);
    }
    assert!(h.events.is_empty());
    assert!(e.entry.is_flagged(74));
    // Any speech playing → nothing.
    let (mut e, mut s, mut h) = (env(), Fake::default(), Hooks::default());
    h.quest_ok = true;
    s.speech = true;
    for t in 1000..1100 {
        entry_tick(&mut e, &mut s, &mut h, 58, t, 0);
    }
    assert!(h.events.is_empty());
    assert!(e.entry.is_flagged(58));
    // L ≥ level count: nothing, not flagged.
    let (mut e, mut s, mut h) = (env(), Fake::default(), Hooks::default());
    h.quest_ok = true;
    let mut inp = input(132, Some(EnvRow::default()));
    inp.level_count = 132;
    for t in 1000..1100 {
        inp.c = t;
        s.t = t;
        e.tick(&mut s, &mut h, &inp);
    }
    assert!(h.events.is_empty());
    assert!(!e.entry.is_flagged(132));
}

// Covers: specs/audio/environment.md §4 text, §4 row1, §4 row2, §4 row3, §4 row4, §4 row5, §4 row6, §4 row7, §4 row8, §4 row9, §4 row10, §4 row11, §4 row12, §4 row13, §4 row14
#[test]
fn entry_table() {
    let rows: [(&[u32], u8, u8); 14] = [
        (&[17], 2, 38),
        (&[34, 35, 36, 37], 6, 40),
        (&[8], 1, 41),
        (&[29, 30, 31], 6, 42),
        (&[26, 27], 3, 43),
        (&[20, 21, 22, 23, 24, 25], 5, 44),
        (&[38], 6, 46),
        (&[2, 3, 4, 5, 6, 7], 1, 47),
        (&[74], 12, 55),
        (&[58], 13, 56),
        (&[110], 31, 76),
        (&[120], 35, 78),
        (&[121], 34, 77),
        (&[132], 36, 79),
    ];
    for (rec, (levels, q, ev)) in ENTRY_RECORDS.iter().zip(rows) {
        assert_eq!((rec.levels, rec.quest, rec.event), (levels, q, ev));
        // Each level plays its record's line on first entry.
        for &l in levels {
            let (mut e, mut s, mut h) = (env(), Fake::default(), Hooks::default());
            h.quest_ok = true;
            for t in 1000..1063 {
                entry_tick(&mut e, &mut s, &mut h, l, t, 0);
            }
            assert_eq!(h.events, vec![ev], "level {l}");
        }
    }
}

// Covers: specs/audio/environment.md §5 r1, §5 r2, §5 r3
#[test]
fn ambience_bed_change_and_day_swap() {
    let (mut e, mut s, mut h) = (env(), Fake::default(), Hooks::default());
    let row = EnvRow {
        day_event: 0,
        night_event: 0,
        ..amb_row()
    };
    // First bed: not a swap (current 0) → group stops, request.
    e.tick(&mut s, &mut h, &input(1, Some(row)));
    assert_eq!(
        s.take(),
        vec![
            Call::Stop52(52, 0),
            Call::Stop72(0, 0),
            req(52, None, 0, 0, 1),
            Call::StopHandle(0),
            Call::StopSongs,
        ]
    );
    assert_eq!((e.ambience.bed, e.ambience.bed_handle), (52, 1));
    // Night: day/night swap → fade old to 0 over 250, new at volume 0 fading to 255.
    let mut inp = input(1, Some(row));
    inp.day_phase = 0;
    e.tick(&mut s, &mut h, &inp);
    assert_eq!(
        s.take(),
        vec![
            Call::Fade(1, 0, 0, 250),
            req(53, None, 0, 0, 2),
            Call::SetVolume(2, 0),
            Call::Fade(2, 255, 0, 250),
            Call::StopHandle(0),
            Call::StopSongs,
        ]
    );
    assert_eq!((e.ambience.bed, e.ambience.bed_handle), (53, 2));
    // Another environment while raining: stops except (a, 64) and its event.
    let other = EnvRow {
        day_ambience: 60,
        night_ambience: 61,
        day_event: 90,
        night_event: 91,
        ..row
    };
    let mut inp = input(2, Some(other));
    inp.day_phase = 0;
    inp.weather_active = true;
    s.t = 1_000_000;
    s.rolls.extend([0, 0]);
    e.tick(&mut s, &mut h, &inp);
    let calls = s.take();
    assert_eq!(
        calls[..3],
        [
            Call::Stop52(61, 64),
            Call::Stop72(91, 0),
            req(61, None, 0, 0, 3)
        ]
    );
    // Bed 0 → current and handle 0.
    let none = EnvRow {
        day_ambience: 0,
        night_ambience: 0,
        ..EnvRow::default()
    };
    e.tick(&mut s, &mut h, &input(3, Some(none)));
    assert_eq!(s.take()[..2], [Call::Stop52(0, 0), Call::Stop72(0, 0)]);
    assert_eq!((e.ambience.bed, e.ambience.bed_handle), (0, 0));
}

// Covers: specs/audio/environment.md §5 r4
#[test]
fn same_bed_day_and_night_keeps_playing() {
    let (mut e, mut s, mut h) = (env(), Fake::default(), Hooks::default());
    let cave = EnvRow {
        day_ambience: 55,
        night_ambience: 55,
        ..EnvRow::default()
    };
    e.tick(&mut s, &mut h, &input(1, Some(cave)));
    s.take();
    let mut inp = input(1, Some(cave));
    inp.day_phase = 5;
    e.tick(&mut s, &mut h, &inp);
    assert_eq!(s.take(), vec![Call::StopHandle(0), Call::StopSongs]);
    assert_eq!(e.ambience.bed_handle, 1);
}

fn rain_input(level: i32) -> TickInput {
    let mut inp = input(1, Some(EnvRow::default()));
    inp.weather_active = true;
    inp.rain_level = level;
    inp
}

fn rain_calls(s: &mut Fake) -> Vec<Call> {
    s.take()
        .into_iter()
        .filter(|c| !matches!(c, Call::StopSongs))
        .collect()
}

// Covers: specs/audio/environment.md §6 r1, §6 r3, §edge-cases-original-bugs r1
#[test]
fn rain_starts_one_tick_late() {
    let (mut e, mut s, mut h) = (env(), Fake::default(), Hooks::default());
    // trunc(0.5 × 255.0) = 127.
    e.tick(&mut s, &mut h, &rain_input(127));
    assert_eq!(
        rain_calls(&mut s),
        vec![req(0, None, 0, 0, 0), Call::SetVolume(0, 127)]
    );
    assert_eq!((e.ambience.rain_handle, e.ambience.rain_prev), (0, 64));
    e.tick(&mut s, &mut h, &rain_input(127));
    assert_eq!(
        rain_calls(&mut s),
        vec![
            req(64, None, 0, 0, 1),
            Call::SetVolume(1, 127),
            Call::SetVolume(1, 127)
        ]
    );
    assert_eq!(e.ambience.rain_handle, 1);
    // Volume moves toward a new target by 6.
    e.tick(&mut s, &mut h, &rain_input(140));
    assert_eq!(rain_calls(&mut s), vec![Call::SetVolume(1, 133)]);
    e.tick(&mut s, &mut h, &rain_input(140));
    assert_eq!(rain_calls(&mut s), vec![Call::SetVolume(1, 139)]);
    e.tick(&mut s, &mut h, &rain_input(140));
    assert_eq!(rain_calls(&mut s), vec![Call::SetVolume(1, 140)]);
}

// Covers: specs/audio/environment.md §6 r3
#[test]
fn rain_fades_out_by_6() {
    let (mut e, mut s, mut h) = (env(), Fake::default(), Hooks::default());
    e.ambience.rain_prev = 64;
    e.tick(&mut s, &mut h, &rain_input(20));
    s.take();
    let mut vols = Vec::new();
    for _ in 0..4 {
        e.tick(&mut s, &mut h, &rain_input(0));
        vols.extend(rain_calls(&mut s));
    }
    assert_eq!(
        vols,
        vec![
            Call::SetVolume(1, 14),
            Call::SetVolume(1, 8),
            Call::SetVolume(1, 2),
            Call::StopHandle(1)
        ]
    );
    assert_eq!(e.ambience.rain_handle, 0);
    // Still raining at 0: no new request while v = 0.
    e.tick(&mut s, &mut h, &rain_input(0));
    assert!(rain_calls(&mut s).is_empty());
}

// Covers: specs/audio/environment.md §6 r2
#[test]
fn rain_stops_when_weather_ends() {
    let (mut e, mut s, mut h) = (env(), Fake::default(), Hooks::default());
    e.ambience.rain_prev = 64;
    e.tick(&mut s, &mut h, &rain_input(100));
    s.take();
    let mut inp = rain_input(100);
    inp.weather_active = false;
    e.tick(&mut s, &mut h, &inp);
    assert_eq!(rain_calls(&mut s), vec![Call::StopHandle(1)]);
    assert_eq!((e.ambience.rain_handle, e.ambience.rain_prev), (0, 0));
}

fn cue_row() -> EnvRow {
    EnvRow {
        day_ambience: 0,
        night_ambience: 0,
        ..amb_row()
    }
}

fn cue_calls(s: &mut Fake) -> Vec<Call> {
    s.take()
        .into_iter()
        .filter(|c| !matches!(c, Call::StopSongs | Call::StopHandle(0)))
        .collect()
}

// Covers: specs/audio/environment.md §7 r1, §7 r2
#[test]
fn cue_gap_on_event_change() {
    let (mut e, mut s, mut h) = (env(), Fake::default(), Hooks::default());
    s.t = 1000;
    // D = 250: gap = 250 + roll(167) − 83; last = T − roll(gap).
    s.rolls.extend([100, 50]);
    e.tick(&mut s, &mut h, &input(1, Some(cue_row())));
    assert_eq!(
        cue_calls(&mut s),
        vec![Call::Roll(167, 100), Call::Roll(267, 50)]
    );
    assert_eq!(e.ambience.gap, 267);
    assert_eq!(e.ambience.last_cue, 950);
    assert_eq!(e.ambience.event_id, 72);
    // Night event differs → new gap.
    let mut inp = input(1, Some(cue_row()));
    inp.day_phase = 4;
    s.rolls.extend([0, 0]);
    e.tick(&mut s, &mut h, &inp);
    assert_eq!(
        cue_calls(&mut s),
        vec![Call::Roll(167, 0), Call::Roll(167, 0)]
    );
    assert_eq!((e.ambience.event_id, e.ambience.gap), (80, 167));
    // ev = 0 → nothing, state kept.
    let quiet = EnvRow {
        day_event: 0,
        ..cue_row()
    };
    s.t = 1_000_000;
    e.tick(&mut s, &mut h, &input(1, Some(quiet)));
    assert!(cue_calls(&mut s).is_empty());
    assert_eq!(e.ambience.event_id, 80);
}

// Covers: specs/audio/environment.md §7 r3, §7 r4
#[test]
fn cue_fires_left_or_right() {
    let (mut e, mut s, mut h) = (env(), Fake::default(), Hooks::default());
    s.t = 1000;
    s.rolls.extend([83, 0]); // gap 250, last 1000
    e.tick(&mut s, &mut h, &input(1, Some(cue_row())));
    s.take();
    s.t = 1249;
    e.tick(&mut s, &mut h, &input(1, Some(cue_row())));
    assert!(cue_calls(&mut s).is_empty());
    // Due, but the request fails: no draws, retried next tick.
    s.t = 1250;
    s.refuse.push(72);
    e.tick(&mut s, &mut h, &input(1, Some(cue_row())));
    assert_eq!(cue_calls(&mut s), vec![req(72, None, 0, 0, 0)]);
    s.refuse.clear();
    s.t = 1251;
    // roll(2) = 0 → left; uniform(450, 750) = 450 + 300; jitter(100) = 5 − 100.
    s.rolls.extend([0, 300, 5, 166]);
    e.tick(&mut s, &mut h, &input(1, Some(cue_row())));
    assert_eq!(
        cue_calls(&mut s),
        vec![
            req(72, None, 0, 0, 1),
            Call::Roll(2, 0),
            Call::Roll(301, 300),
            Call::Roll(201, 5),
            Call::SetPosition(1, -750, -95, 640),
            Call::Roll(167, 166),
        ]
    );
    assert_eq!((e.ambience.last_cue, e.ambience.gap), (1251, 333));
    // Right side.
    s.t = 1251 + 333;
    s.rolls.extend([1, 0, 200, 0]);
    e.tick(&mut s, &mut h, &input(1, Some(cue_row())));
    assert!(cue_calls(&mut s).contains(&Call::SetPosition(2, 450, 100, 640)));
}

// Covers: specs/audio/environment.md §7 r5
#[test]
fn ambience_runs_before_music() {
    let (mut e, mut s, mut h) = (env(), Fake::default(), Hooks::default());
    let row = EnvRow {
        song: SONG_A,
        ..amb_row()
    };
    e.tick(&mut s, &mut h, &input(1, Some(row)));
    let calls = s.take();
    let bed = calls.iter().position(|c| *c == req(52, None, 0, 0, 1));
    let song = calls
        .iter()
        .position(|c| *c == req(SONG_A, None, FLAG_NO_FADE_IN, 0, 2));
    assert!(bed.is_some() && song.is_some() && bed < song, "{calls:?}");
}

// Covers: specs/audio/environment.md §8
#[test]
fn sample_pins_on_level_change() {
    let (mut e, mut s, mut h) = (env(), Fake::default(), Hooks::default());
    let row = cue_row();
    s.rolls.extend([0, 0]);
    assert_eq!(
        e.tick(&mut s, &mut h, &input(1, Some(row))),
        Some(SamplePins { materials: [2, 3] })
    );
    assert_eq!(e.tick(&mut s, &mut h, &input(1, Some(row))), None);
    assert_eq!(
        e.tick(&mut s, &mut h, &input(2, None)),
        Some(SamplePins { materials: [0, 0] })
    );
}

// Covers: specs/audio/environment.md §2 text, §3 text, §5 text, §6 text, §7 text
#[test]
fn state_variables_start_at_zero() {
    // The §2 music state (cur, last level, Tl, Ts, last announced, one
    // resume value per song id of the range), the §3 stinger state and
    // the §5–§7 ambience, rain and event-cue state all start at 0.
    let e = Environment::new(SongRange::LIVE);
    let m = &e.music;
    assert_eq!(
        (m.cur, m.last_level, m.tl, m.ts, m.last_announced),
        (0, 0, 0, 0, 0)
    );
    for id in 4657..=4684 {
        assert_eq!(m.resume(id), 0, "song {id}");
    }
    assert_eq!(m.stinger, Stinger::default());
    assert!(!m.stinger.active && !m.stinger.m_pending && !m.stinger.s_pending);
    let a = e.ambience;
    assert_eq!((a.bed, a.bed_handle), (0, 0));
    assert_eq!((a.rain_handle, a.rain_prev), (0, 0));
    assert_eq!((a.event_id, a.gap, a.last_cue), (0, 0, 0));
}
