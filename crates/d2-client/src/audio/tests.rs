// Spec: specs/client/audio.md
//! Test vectors of `audio.md` §A2–§A5 on synthetic samples (no game files,
//! no audio device).

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use super::log::header;
use super::output::{to_device, MixerStream};
use super::*;
use bevy::audio::{Decodable, Source};

#[derive(Default)]
struct FakeBank {
    sounds: BTreeMap<u32, (Arc<str>, Option<Arc<Sound>>)>,
}

impl FakeBank {
    fn with(mut self, id: u32, file: &str, sound: Option<Sound>) -> Self {
        self.sounds.insert(id, (file.into(), sound.map(Arc::new)));
        self
    }
}

impl SoundBank for FakeBank {
    fn file(&self, id: SoundId) -> Option<Arc<str>> {
        self.sounds.get(&id.0).map(|(f, _)| Arc::clone(f))
    }

    fn samples(&self, id: SoundId) -> Option<Arc<Sound>> {
        self.sounds.get(&id.0).and_then(|(_, s)| s.clone())
    }
}

fn engine(bank: FakeBank) -> AudioEngine {
    AudioEngine::new(Box::new(bank), Box::new(UnityGain), Box::new(Unlimited))
}

fn params(vol: i32, pan: i32, looped: bool) -> VoiceParams {
    VoiceParams {
        vol,
        pan,
        looped,
        priority: 0,
        group: 0,
    }
}

fn start(tick: u32, sound: u32, p: VoiceParams, cause: &str) -> Cue {
    Cue::Start(Trigger {
        tick,
        source: TriggerSource::Sim,
        sound: SoundId(sound),
        params: p,
        cause: cause.to_owned(),
    })
}

fn constant(rate: u32, value: i16, frames: usize) -> Sound {
    Sound::new(rate, 1, vec![value; frames]).unwrap()
}

/// FNV-1a 64 over the little-endian bytes of the samples.
fn fnv1a(samples: &[i16]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for s in samples {
        for b in s.to_le_bytes() {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    h
}

fn files(e: &AudioEngine) -> Vec<(u32, VoiceKind, String)> {
    e.log()
        .events
        .iter()
        .map(|v| (v.tick, v.kind, v.file.clone()))
        .collect()
}

// --- §A2 triggers -------------------------------------------------------

// Covers: specs/client/audio.md §a2-triggers
#[test]
fn two_triggers_in_one_tick_start_in_emission_order() {
    let bank = FakeBank::default()
        .with(1, "a.wav", Some(constant(44_100, 1, 4)))
        .with(2, "b.wav", Some(constant(44_100, 2, 4)));
    let mut e = engine(bank);
    let b = e.queue_mut().push(start(5, 2, params(255, 0, false), "b"));
    let a = e.queue_mut().push(start(5, 1, params(255, 0, false), "a"));
    e.present(5).unwrap();
    e.mix_block();
    assert_eq!(
        files(&e),
        [
            (5, VoiceKind::Start, "b.wav".to_owned()),
            (5, VoiceKind::Start, "a.wav".to_owned())
        ]
    );
    assert!(b < a);
}

// Covers: specs/client/audio.md §a2-triggers, §a3-scheduler-and-clock
#[test]
fn queue_orders_by_tick_then_emission() {
    let mut q = TriggerQueue::new();
    let p = params(1, 0, false);
    let i7a = q.push(start(7, 1, p, "7a"));
    let i3 = q.push(start(3, 1, p, "3"));
    let i9 = q.push(start(9, 1, p, "9"));
    let i7b = q.push(start(7, 1, p, "7b"));
    let ids: Vec<_> = q.take_due(8).into_iter().map(|(id, _)| id).collect();
    assert_eq!(ids, [i3, i7a, i7b]);
    assert_eq!(q.len(), 1);
    assert_eq!(q.take_due(9)[0].0, i9);
    assert!(q.is_empty());
}

struct ScriptedSource(Vec<Cue>);

impl CueSource for ScriptedSource {
    fn drain_cues(&mut self, queue: &mut TriggerQueue) {
        for c in self.0.drain(..) {
            queue.push(c);
        }
    }
}

// Covers: specs/client/audio.md §a2-triggers
#[test]
fn cue_source_seam_feeds_the_queue_in_order() {
    let bank = FakeBank::default().with(1, "a.wav", Some(constant(44_100, 1, 4)));
    let mut e = engine(bank);
    let mut src = ScriptedSource(vec![
        start(1, 1, params(10, 0, false), "x"),
        start(1, 1, params(20, 0, false), "y"),
    ]);
    e.pump(&mut src);
    e.present(1).unwrap();
    e.mix_block();
    let causes: Vec<_> = e.log().events.iter().map(|v| v.cause.as_str()).collect();
    assert_eq!(causes, ["x", "y"]);
}

// --- §A3 scheduler ------------------------------------------------------

// Covers: specs/client/audio.md §a3-scheduler-and-clock
#[test]
fn trigger_at_tick_10_starts_in_block_after_tick_10_is_presented() {
    let bank = FakeBank::default().with(1, "a.wav", Some(constant(44_100, 1000, 4096)));
    let mut e = engine(bank);
    e.queue_mut().push(start(10, 1, params(255, 0, false), "t"));
    e.present(9).unwrap();
    assert!(e.mix_block().iter().all(|&s| s == 0));
    assert!(e.log().events.is_empty());
    e.present(10).unwrap();
    let block = e.mix_block();
    assert_eq!(block[0], 1000);
    assert_eq!(files(&e), [(10, VoiceKind::Start, "a.wav".to_owned())]);
}

// Covers: specs/client/audio.md §a3-scheduler-and-clock
#[test]
fn ticks_never_go_back_and_late_cues_are_reported() {
    let bank = FakeBank::default().with(1, "a.wav", Some(constant(44_100, 1, 4)));
    let mut e = engine(bank);
    e.present(10).unwrap();
    assert_eq!(
        e.present(9),
        Err(AudioError::TickBackwards {
            tick: 9,
            previous: 10
        })
    );
    e.queue_mut().push(start(8, 1, params(1, 0, false), "late"));
    e.present(11).unwrap();
    e.mix_block();
    assert_eq!(
        e.take_errors(),
        [AudioError::LateCue {
            tick: 8,
            presented: 10
        }]
    );
    // Still started, logged with its own tick.
    assert_eq!(e.log().events[0].tick, 8);
}

// Covers: specs/client/audio.md §a3-scheduler-and-clock, §a5-voice-log-exactness-check-2
#[test]
fn stops_are_tick_stamped_and_logged() {
    let bank = FakeBank::default()
        .with(1, "a.wav", Some(constant(44_100, 100, 8)))
        .with(2, "b.wav", Some(constant(44_100, 200, 8)));
    let mut e = engine(bank);
    let a = e.queue_mut().push(start(1, 1, params(30, -4, true), "a"));
    e.queue_mut().push(start(1, 2, params(40, 4, true), "b"));
    e.present(1).unwrap();
    e.mix_block();
    e.queue_mut().push(Cue::Stop(Stop {
        tick: 2,
        target: StopTarget::Voice(a),
        cause: "death".into(),
    }));
    e.present(2).unwrap();
    let block = e.mix_block();
    assert_eq!(block[0], 200);
    e.queue_mut().push(Cue::Stop(Stop {
        tick: 3,
        target: StopTarget::All,
        cause: "area".into(),
    }));
    // Stopping an already stopped voice logs nothing.
    e.queue_mut().push(Cue::Stop(Stop {
        tick: 3,
        target: StopTarget::Voice(a),
        cause: "again".into(),
    }));
    e.present(3).unwrap();
    assert!(e.mix_block().iter().all(|&s| s == 0));
    let stops: Vec<_> = e
        .log()
        .events
        .iter()
        .filter(|v| v.kind == VoiceKind::Stop)
        .map(|v| (v.tick, v.file.as_str(), v.vol, v.pan, v.looped))
        .collect();
    assert_eq!(
        stops,
        [(2, "a.wav", 30, -4, true), (3, "b.wav", 40, 4, true)]
    );
}

// Covers: specs/client/audio.md §a5-voice-log-exactness-check-2
#[test]
fn param_change_is_logged_with_new_values() {
    let bank = FakeBank::default().with(1, "a.wav", Some(constant(44_100, 1, 8)));
    let mut e = engine(bank);
    let a = e.queue_mut().push(start(1, 1, params(30, 0, true), "a"));
    e.queue_mut().push(Cue::Param(ParamChange {
        tick: 2,
        target: a,
        vol: 50,
        pan: -7,
        cause: "move".into(),
    }));
    e.present(2).unwrap();
    e.mix_block();
    let last = e.log().events.last().unwrap();
    assert_eq!(
        (last.tick, last.kind, last.vol, last.pan, last.looped),
        (2, VoiceKind::Param, 50, -7, true)
    );
    assert_eq!(e.voices()[0].params().vol, 50);
}

struct StealFirst;

impl VoicePolicy for StealFirst {
    fn admit(&mut self, _: &Trigger, playing: &[Voice]) -> Admit {
        match playing.first() {
            Some(v) => Admit::Steal(v.id()),
            None => Admit::Start,
        }
    }
}

// Covers: specs/client/audio.md §a3-scheduler-and-clock
#[test]
fn voice_policy_hook_can_steal() {
    let bank = FakeBank::default()
        .with(1, "a.wav", Some(constant(44_100, 1, 8)))
        .with(2, "b.wav", Some(constant(44_100, 2, 8)));
    let mut e = AudioEngine::new(Box::new(bank), Box::new(UnityGain), Box::new(StealFirst));
    e.queue_mut().push(start(1, 1, params(1, 0, true), "a"));
    e.queue_mut().push(start(1, 2, params(1, 0, true), "b"));
    e.present(1).unwrap();
    e.mix_block();
    assert_eq!(
        files(&e),
        [
            (1, VoiceKind::Start, "a.wav".to_owned()),
            (1, VoiceKind::Stop, "a.wav".to_owned()),
            (1, VoiceKind::Start, "b.wav".to_owned()),
        ]
    );
    assert_eq!(e.voices().len(), 1);
}

// Covers: specs/client/audio.md §edge-cases-original-bugs
#[test]
fn missing_file_is_a_logged_error_start() {
    let bank = FakeBank::default().with(1, "gone.wav", None);
    let mut e = engine(bank);
    e.queue_mut().push(start(1, 1, params(9, 0, false), "c1"));
    e.queue_mut().push(start(1, 7, params(9, 0, false), "c2"));
    e.present(1).unwrap();
    assert!(e.mix_block().iter().all(|&s| s == 0));
    let log: Vec<_> = e
        .log()
        .events
        .iter()
        .map(|v| (v.kind, v.file.as_str(), v.error))
        .collect();
    assert_eq!(
        log,
        [
            (VoiceKind::Start, "gone.wav", true),
            (VoiceKind::Start, "#7", true)
        ]
    );
    assert_eq!(
        e.take_errors(),
        [
            AudioError::MissingFile {
                file: "gone.wav".into(),
                cause: "c1".into()
            },
            AudioError::UnknownSound {
                sound: SoundId(7),
                cause: "c2".into()
            }
        ]
    );
}

// --- §A4 mixer ----------------------------------------------------------

fn voice(sound: Sound, looped: bool, gains: Gains) -> Voice {
    Voice::new(
        CueId(0),
        "x.wav".into(),
        Arc::new(sound),
        params(255, 0, looped),
        gains,
    )
}

fn unity() -> Gains {
    Gains::new(GAIN_UNITY, GAIN_UNITY, GAIN_UNITY).unwrap()
}

// Covers: specs/client/audio.md §a4-mixer
#[test]
fn one_voice_constant_1000_at_unity_placeholder() {
    // The spec vector's expected output waits for the §B3 tables; this
    // pins the placeholder curve: unity gain passes samples through.
    let mut v = vec![voice(constant(44_100, 1000, 2048), false, unity())];
    assert!(mix(&mut v).iter().all(|&s| s == 1000));
}

// Covers: specs/client/audio.md §a4-mixer
#[test]
fn gain_is_q8_product_shifted_16() {
    let g = Gains::new(128, 64, 256).unwrap();
    let mut v = vec![voice(constant(44_100, 1000, 2048), false, g)];
    let out = mix(&mut v);
    // (1000·128·64) >> 16 = 125; (1000·128·256) >> 16 = 500.
    assert_eq!((out[0], out[1]), (125, 500));
    // Negative values shift arithmetically (toward −∞).
    let mut v = vec![voice(constant(44_100, -1, 2048), false, g)];
    assert_eq!(mix(&mut v)[0], -1);
    // Extremes stay inside i32.
    let mut v = vec![voice(constant(44_100, i16::MIN, 2048), false, unity())];
    assert_eq!(mix(&mut v)[0], i16::MIN);
    assert_eq!(Gains::new(257, 0, 0), Err(AudioError::GainRange(257)));
    assert_eq!(Gains::new(0, -1, 0), Err(AudioError::GainRange(-1)));
}

// Covers: specs/client/audio.md §a4-mixer
#[test]
fn two_voices_summing_past_32767_saturate() {
    let mut v = vec![
        voice(constant(44_100, 20_000, 2048), false, unity()),
        voice(constant(44_100, 20_000, 2048), false, unity()),
    ];
    assert!(mix(&mut v).iter().all(|&s| s == 32767));
    let mut v = vec![
        voice(constant(44_100, -20_000, 2048), false, unity()),
        voice(constant(44_100, -20_000, 2048), false, unity()),
    ];
    assert!(mix(&mut v).iter().all(|&s| s == -32768));
}

// Covers: specs/client/audio.md §a4-mixer
#[test]
fn rate_22050_outputs_each_sample_twice() {
    let samples: Vec<i16> = (0..600).collect();
    let mut v = vec![voice(
        Sound::new(22_050, 1, samples).unwrap(),
        false,
        unity(),
    )];
    let out = mix(&mut v);
    let left: Vec<i16> = out.iter().step_by(2).copied().collect();
    let expect: Vec<i16> = (0..256).flat_map(|s| [s, s]).collect();
    assert_eq!(left, expect);
}

// Covers: specs/client/audio.md §a4-mixer
#[test]
fn stereo_sources_keep_their_sides() {
    let samples: Vec<i16> = (0..1024).map(|i| if i % 2 == 0 { 7 } else { -7 }).collect();
    let mut v = vec![voice(
        Sound::new(44_100, 2, samples).unwrap(),
        false,
        unity(),
    )];
    let out = mix(&mut v);
    assert!(out.chunks(2).all(|f| f == [7, -7]));
}

// Covers: specs/client/audio.md §a4-mixer
#[test]
fn one_shot_ends_and_looped_wraps() {
    let ramp: Vec<i16> = (1..=300).collect();
    let mut v = vec![voice(
        Sound::new(44_100, 1, ramp.clone()).unwrap(),
        false,
        unity(),
    )];
    let out = mix(&mut v);
    assert_eq!(out[2 * 299], 300);
    assert!(out[2 * 300..].iter().all(|&s| s == 0));
    assert!(v.is_empty(), "one-shot voice removed after its last frame");

    let mut v = vec![voice(Sound::new(44_100, 1, ramp).unwrap(), true, unity())];
    let out = mix(&mut v);
    assert_eq!((out[2 * 299], out[2 * 300], out[2 * 511]), (300, 1, 212));
    assert_eq!(v.len(), 1);
}

// Covers: specs/client/audio.md §a1-decode-path r2
#[test]
fn sound_input_is_strict() {
    assert_eq!(Sound::new(0, 1, vec![0]), Err(AudioError::ZeroRate));
    assert_eq!(Sound::new(1, 3, vec![0; 3]), Err(AudioError::Channels(3)));
    assert_eq!(
        Sound::new(1, 2, vec![0; 3]),
        Err(AudioError::PartialFrame {
            samples: 3,
            channels: 2
        })
    );
    assert_eq!(Sound::new(1, 1, vec![]), Err(AudioError::Empty));
}

/// Scripted voice set: rates 11,025 / 22,050 / 32,000 (non-integer step)
/// / 44,100, mono and stereo, one-shot and looped, partial gains, starts,
/// a stop and a param change over several ticks and blocks.
fn scripted_run() -> (Vec<i16>, String) {
    struct HalfPan;
    impl GainCurve for HalfPan {
        // A test curve (not the original's): vol and pan pass through as
        // Q8 indices, pan > 0 attenuates the left side.
        fn gains(&self, vol: i32, pan: i32) -> Result<Gains, AudioError> {
            let (l, r) = if pan > 0 {
                (GAIN_UNITY - pan, GAIN_UNITY)
            } else {
                (GAIN_UNITY, GAIN_UNITY + pan)
            };
            Gains::new(vol, l, r)
        }
    }
    let saw: Vec<i16> = (0..700).map(|i| ((i * 97) % 4001 - 2000) as i16).collect();
    let tone: Vec<i16> = (0..1200)
        .flat_map(|i| [(i * 31 % 1201 - 600) as i16, (i * 53 % 901 - 450) as i16])
        .collect();
    let bank = FakeBank::default()
        .with(
            1,
            "data\\global\\sfx\\a.wav",
            Some(Sound::new(11_025, 1, saw.clone()).unwrap()),
        )
        .with(
            2,
            "data\\global\\sfx\\b.wav",
            Some(Sound::new(22_050, 2, tone).unwrap()),
        )
        .with(
            3,
            "data\\global\\sfx\\c.wav",
            Some(Sound::new(32_000, 1, saw.clone()).unwrap()),
        )
        .with(
            4,
            "data\\global\\sfx\\d.wav",
            Some(Sound::new(44_100, 1, saw).unwrap()),
        );
    let mut e = AudioEngine::new(Box::new(bank), Box::new(HalfPan), Box::new(Unlimited));
    let q = e.queue_mut();
    q.push(start(0, 1, params(256, 0, true), "loop a"));
    let b = q.push(start(1, 2, params(200, 40, false), "b"));
    q.push(start(1, 3, params(256, -100, false), "c"));
    q.push(start(3, 4, params(90, 0, true), "d"));
    q.push(Cue::Param(ParamChange {
        tick: 4,
        target: b,
        vol: 100,
        pan: -30,
        cause: "move".into(),
    }));
    q.push(Cue::Stop(Stop {
        tick: 6,
        target: StopTarget::All,
        cause: "area".into(),
    }));
    let mut out = Vec::new();
    for t in 0..8 {
        e.present(t).unwrap();
        out.extend_from_slice(&e.mix_block());
    }
    assert!(e.take_errors().is_empty());
    (out, e.log().to_jsonl())
}

// Covers: specs/client/audio.md §a4-mixer, §a5-voice-log-exactness-check-2
#[test]
fn scripted_voice_set_golden_hash() {
    let (out, log) = scripted_run();
    assert_eq!(scripted_run(), (out.clone(), log.clone()), "deterministic");
    assert_eq!(out.len(), 8 * BLOCK_SAMPLES);
    assert_eq!(fnv1a(&out), GOLDEN_MIX, "mix hash {:#018x}", fnv1a(&out));
    assert_eq!(
        fnv1a_bytes(log.as_bytes()),
        GOLDEN_LOG,
        "log hash {:#018x}\n{log}",
        fnv1a_bytes(log.as_bytes())
    );
}

const GOLDEN_MIX: u64 = 0xee78_5cfa_df1b_00f9;
const GOLDEN_LOG: u64 = 0xd000_e570_09ec_a851;

fn fnv1a_bytes(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

// Covers: specs/client/audio.md §a4-mixer
#[test]
fn golden_hash_catches_one_sample_change() {
    // M08: the golden check fails on a one-sample perturbation.
    let (mut out, _) = scripted_run();
    out[777] = out[777].wrapping_add(1);
    assert_ne!(fnv1a(&out), GOLDEN_MIX);
}

// --- §A4 output edge ----------------------------------------------------

// Covers: specs/client/audio.md §a4-mixer
#[test]
fn device_conversion_is_exact_division() {
    assert_eq!(to_device(i16::MIN), -1.0);
    assert_eq!(to_device(16384), 0.5);
    assert_eq!(to_device(1), 1.0 / 32768.0);
    for s in [i16::MIN, -12345, -1, 0, 1, 777, i16::MAX] {
        assert_eq!((to_device(s) * 32768.0) as i16, s);
    }
}

// Covers: specs/client/audio.md §a4-mixer
#[test]
fn decoder_streams_engine_blocks_without_a_device() {
    let bank = FakeBank::default().with(1, "a.wav", Some(constant(44_100, 16384, 600)));
    let mut eng = engine(bank);
    eng.queue_mut()
        .push(start(0, 1, params(255, 0, false), "a"));
    eng.present(0).unwrap();
    let shared = Arc::new(Mutex::new(eng));
    let mut d = MixerStream::new(Arc::clone(&shared)).decoder();
    assert_eq!(d.channels().get(), 2);
    assert_eq!(d.sample_rate().get(), OUTPUT_RATE);
    let first: Vec<f32> = d.by_ref().take(4).collect();
    assert_eq!(first, [0.5; 4]);
    let rest: Vec<f32> = d.take(2 * BLOCK_SAMPLES - 4).collect();
    // 600 frames: frames 600.. of block 2 are silent.
    assert_eq!(rest[2 * 599 - 4], 0.5);
    assert_eq!(rest[2 * 600 - 4], 0.0);
    assert_eq!(shared.lock().unwrap().log().events.len(), 1);
}

// --- §A5 voice log ------------------------------------------------------

// Covers: specs/client/audio.md §a5-voice-log-exactness-check-2
#[test]
fn audio_log_header() {
    assert_eq!(header(), r#"{"format":"d2rs-audio-log","version":1}"#);
    assert_eq!(
        VoiceLog::default().to_jsonl(),
        "{\"format\":\"d2rs-audio-log\",\"version\":1}\n"
    );
}

fn sample_log() -> VoiceLog {
    VoiceLog {
        events: vec![
            VoiceEvent {
                tick: 10,
                kind: VoiceKind::Start,
                file: "data\\global\\sfx\\a \"q\".wav".into(),
                vol: 255,
                pan: -64,
                looped: false,
                error: false,
                cause: "skill\n0x2c".into(),
            },
            VoiceEvent {
                tick: 12,
                kind: VoiceKind::Param,
                file: "b.wav".into(),
                vol: 0,
                pan: 64,
                looped: true,
                error: false,
                cause: String::new(),
            },
            VoiceEvent {
                tick: u32::MAX,
                kind: VoiceKind::Stop,
                file: "b.wav".into(),
                vol: i32::MIN,
                pan: i32::MAX,
                looped: true,
                error: true,
                cause: "ü\u{1}".into(),
            },
        ],
    }
}

// Covers: specs/client/audio.md §a5-voice-log-exactness-check-2
#[test]
fn log_round_trips() {
    let log = sample_log();
    let text = log.to_jsonl();
    assert_eq!(
        text.lines().nth(1).unwrap(),
        r#"{"tick":10,"kind":"start","file":"data\\global\\sfx\\a \"q\".wav","vol":255,"pan":-64,"looped":false,"error":false,"cause":"skill\u000a0x2c"}"#
    );
    assert_eq!(VoiceLog::parse(&text), Ok(log));
}

// Covers: specs/client/audio.md §a5-voice-log-exactness-check-2
#[test]
fn log_parser_is_strict() {
    let good = sample_log().to_jsonl();
    assert_eq!(VoiceLog::parse(""), Err(LogError::Empty));
    assert_eq!(
        VoiceLog::parse(good.trim_end()),
        Err(LogError::NoFinalNewline)
    );
    assert!(matches!(
        VoiceLog::parse(&good.replace("\"version\":1", "\"version\":2")),
        Err(LogError::Header { .. })
    ));
    let line = |bad: &str| match VoiceLog::parse(&good.replacen("\"tick\":10,", bad, 1)) {
        Err(LogError::Record { line, .. }) => line,
        other => panic!("{bad:?} accepted: {other:?}"),
    };
    for bad in [
        "\"tick\":010,",
        "\"tick\":-1,",
        "\"tick\": 10,",
        "\"tick\":10,\"x\":1,",
        "\"tick\":4294967296,",
    ] {
        assert_eq!(line(bad), 2);
    }
    let rec = |s: &str| VoiceLog::parse(&format!("{}\n{s}\n", header()));
    let base = r#"{"tick":1,"kind":"start","file":"f","vol":0,"pan":0,"looped":false,"error":false,"cause":"c"}"#;
    assert!(rec(base).is_ok());
    for bad in [
        base.replace("start", "begin"),
        base.replace("\"f\"", "\"\\n\""),
        base.replace("\"f\"", "\"\\u0041\""),
        base.replace("\"f\"", "\"\\u000A\""),
        base.replace("\"f\"", "\"\u{1}\""),
        base.replace("false,\"error\"", "0,\"error\""),
        base.replace("\"vol\":0", "\"vol\":-0"),
        base.replace("\"vol\":0", "\"vol\":2147483648"),
        format!("{base} "),
        format!("{base}\n"),
    ] {
        assert!(rec(&bad).is_err(), "accepted {bad:?}");
    }
}

// Covers: specs/client/audio.md §a5-voice-log-exactness-check-2
#[test]
fn compare_logs_ignores_cause_and_error() {
    let ours = sample_log();
    let mut theirs = sample_log();
    for e in &mut theirs.events {
        e.cause = "other".into();
        e.error = !e.error;
    }
    assert!(compare_logs(&ours, &theirs).is_empty());
}

// Covers: specs/client/audio.md §a5-voice-log-exactness-check-2
#[test]
fn compare_logs_reports_exactly_the_perturbed_fields() {
    // M08: each compared field, changed alone, is reported alone.
    let ours = sample_log();
    type Perturb = (&'static str, fn(&mut VoiceEvent));
    let perturbations: [Perturb; 6] = [
        ("tick", |e| e.tick ^= 1),
        ("kind", |e| e.kind = VoiceKind::Param),
        ("file", |e| e.file.push('x')),
        ("vol", |e| e.vol = e.vol.wrapping_add(1)),
        ("pan", |e| e.pan = e.pan.wrapping_sub(1)),
        ("looped", |e| e.looped = !e.looped),
    ];
    for (field, perturb) in perturbations {
        let mut theirs = sample_log();
        perturb(&mut theirs.events[0]);
        assert_eq!(
            compare_logs(&ours, &theirs),
            [LogMismatch::Field {
                index: 0,
                fields: vec![field]
            }]
        );
    }
    let mut theirs = sample_log();
    theirs.events.pop();
    assert_eq!(
        compare_logs(&ours, &theirs),
        [LogMismatch::Length { ours: 3, theirs: 2 }]
    );
}
