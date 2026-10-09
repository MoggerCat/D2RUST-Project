// Spec: specs/client/audio.md
//! Audio core: trigger queue, tick scheduler, integer mixer and voice log
//! (`audio.md` §A2–§A5). Everything here except [`output`] is plain Rust
//! (no Bevy, no float, no I/O) so it runs in CI without an audio device;
//! [`output`] is the only Bevy/rodio edge.
//!
//! Original behavior lives behind hooks, each owned by its spec:
//! - which events make sounds and their parameters ([`CueSource`]):
//!   `audio/triggers.md`, `audio/triggers-2.md` §21, `audio/environment.md`
//!   ([`driver::SoundDriver`]);
//! - sound ids to files, variants and their RNG ([`SoundBank`]):
//!   `audio/sound-table.md` §1–§7, §13, `audio/sound-table-2.md` §16–§17
//!   ([`sound_table`]);
//! - volume and pan curves ([`GainCurve`]): `audio/sound-table.md` §8.3
//!   ([`sound_table::DeviceGain`]; [`UnityGain`] is the neutral curve of
//!   the hook-only play mode);
//! - voice limits, stealing, repeat suppression ([`VoicePolicy`]):
//!   `audio/sound-table.md` §6–§7, decided by [`sound_table::SoundSystem`]
//!   before a cue is queued, so the core admits every start
//!   ([`Unlimited`]);
//! - the WAV decode giving [`Sound`] ([`pool::D2Wav`], `formats/wav.md`
//!   §5; the pool reading and keeping each file once is ours).

pub mod calls;
pub mod driver;
pub mod environment;
pub mod log;
pub mod mixer;
pub mod output;
pub mod pool;
pub mod sound_table;
pub mod triggers;
pub mod unit_feed;

#[cfg(test)]
mod tests;

use std::sync::Arc;

pub use log::{compare_logs, LogError, LogMismatch, VoiceEvent, VoiceKind, VoiceLog};
pub use mixer::{
    mix, GainCurve, Gains, Sound, UnityGain, Voice, BLOCK_FRAMES, BLOCK_SAMPLES, GAIN_UNITY,
    OUTPUT_RATE,
};
pub use pool::{sound_bytes, D2Wav, SoundPool, SoundPoolError, WavDecoder};

/// Errors of the audio core. None is swallowed: each is returned or kept in
/// [`AudioEngine::take_errors`] (M07).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AudioError {
    #[error("sound has rate 0")]
    ZeroRate,
    #[error("sound has {0} channels; 1 or 2 supported")]
    Channels(u16),
    #[error("sound has {samples} samples, not a multiple of {channels} channels")]
    PartialFrame { samples: usize, channels: u16 },
    #[error("sound has no samples")]
    Empty,
    #[error("gain component {0} outside 0..={GAIN_UNITY}")]
    GainRange(i32),
    #[error("vol {vol} / pan {pan} outside the gain curve's domain")]
    GainDomain { vol: i32, pan: i32 },
    #[error("tick {tick} presented after tick {previous}")]
    TickBackwards { tick: u32, previous: u32 },
    #[error("cue at tick {tick} queued after tick {presented} was presented")]
    LateCue { tick: u32, presented: u32 },
    #[error("sound {sound:?} has no file (cause {cause})")]
    UnknownSound { sound: SoundId, cause: String },
    #[error("sound file {file} is missing (cause {cause})")]
    MissingFile { file: String, cause: String },
    #[error("voice policy stole {0:?}, which is not playing")]
    BadSteal(CueId),
}

/// A sound as the sound table names it (`audio.md` §A2). Its meaning
/// (row of `sounds.txt` or otherwise) is §B3.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SoundId(pub u32);

/// Which tick domain a trigger's tick belongs to (`audio.md` §A2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TriggerSource {
    /// Simulation event, stamped with its sim tick.
    Sim,
    /// UI event, stamped with the client tick.
    Ui,
}

/// Integer voice parameters (`audio.md` §A2). Ranges and meaning are §B3;
/// the mixer reads `vol` and `pan` only through a [`GainCurve`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VoiceParams {
    pub vol: i32,
    pub pan: i32,
    pub looped: bool,
    pub priority: i32,
    pub group: i32,
}

/// Ours, for debugging only: never compared (`audio.md` §A5).
pub type CauseTag = String;

/// One sound start (`audio.md` §A2).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Trigger {
    pub tick: u32,
    pub source: TriggerSource,
    pub sound: SoundId,
    pub params: VoiceParams,
    pub cause: CauseTag,
}

/// Identity of a queued cue, assigned in emission order by
/// [`TriggerQueue::push`]. A started voice keeps its trigger's id, so stops
/// and parameter changes name it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CueId(pub u64);

/// What a stop ends.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StopTarget {
    Voice(CueId),
    /// Every playing voice (area change and the like).
    All,
}

/// A tick-stamped stop (`audio.md` §A3).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stop {
    pub tick: u32,
    pub target: StopTarget,
    pub cause: CauseTag,
}

/// A tick-stamped volume/pan change of a playing voice (`audio.md` §A5
/// kind `Param`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParamChange {
    pub tick: u32,
    pub target: CueId,
    pub vol: i32,
    pub pan: i32,
    pub cause: CauseTag,
}

/// Anything the scheduler starts at a tick.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Cue {
    Start(Trigger),
    Stop(Stop),
    Param(ParamChange),
    /// Device state of a started voice that the voice log does not
    /// record.
    Device(DeviceChange),
}

impl Cue {
    pub fn tick(&self) -> u32 {
        match self {
            Cue::Start(t) => t.tick,
            Cue::Stop(s) => s.tick,
            Cue::Param(p) => p.tick,
            Cue::Device(d) => d.tick,
        }
    }
}

/// Device-side state of a playing voice (`sound-table.md` §7 r4, r8,
/// §8.3 r3): where it starts, where it loops to, and the occlusion the
/// device applies to its volume. Not a voice-log record: the log keeps the
/// volume and pan sent (§8.2), so these change only the mix.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeviceChange {
    pub tick: u32,
    pub target: CueId,
    /// First frame played (`Some` only right after the start: the stream
    /// start offset of §7 r8).
    pub start_frame: Option<u64>,
    /// Frame a looping voice wraps to (§7 r4: `Block 1` × 2 bytes).
    pub loop_start: Option<u64>,
    /// Occlusion (request +0x20, §6.4) as `f32` bits.
    pub occlusion: u32,
}

/// Pending cues in emission order (`audio.md` §A2). Cues are released by
/// tick; within a tick, in the order they were pushed (stable).
#[derive(Debug, Default)]
pub struct TriggerQueue {
    next: u64,
    pending: Vec<(CueId, Cue)>,
}

impl TriggerQueue {
    pub fn new() -> Self {
        Self::default()
    }

    /// Queue a cue; returns its id (emission order).
    pub fn push(&mut self, cue: Cue) -> CueId {
        let id = CueId(self.next);
        self.next += 1;
        self.pending.push((id, cue));
        id
    }

    pub fn len(&self) -> usize {
        self.pending.len()
    }

    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }

    /// Remove and return every cue with `tick <= t`, ordered by tick and,
    /// within a tick, by emission order.
    pub fn take_due(&mut self, t: u32) -> Vec<(CueId, Cue)> {
        let (mut due, keep): (Vec<_>, Vec<_>) = std::mem::take(&mut self.pending)
            .into_iter()
            .partition(|(_, c)| c.tick() <= t);
        self.pending = keep;
        // Stable sort: emission order holds within a tick.
        due.sort_by_key(|(_, c)| c.tick());
        due
    }
}

/// The narrow seam through which sound-bearing sim and UI events reach the
/// audio core: its rule functions (one per cause class) turn snapshots and
/// events into cues and push them in emission order.
///
/// Which events make requests and with which parameters is
/// `audio/triggers.md` (per cause class), `audio/triggers-2.md` §21 (the
/// inputs each rule reads) and `audio/environment.md` (music, ambience,
/// rain, level lines); [`driver::SoundDriver`] implements it over the
/// sound layer.
pub trait CueSource {
    /// Push every cue produced since the last call, in emission order.
    fn drain_cues(&mut self, queue: &mut TriggerQueue);
}

/// Sound ids to files and decoded samples.
///
/// The id → path rule, groups and variants are `audio/sound-table.md`
/// §1, §3, §4 (`sound_table::SoundPaths`, `SoundTableData::pick_variant`
/// on the client seed of §4 r5); the cache that decides when a sample is
/// available is `audio/sound-table-2.md` §16–§17; the decode giving
/// [`Sound`] is `formats/wav.md` §5 ([`pool::D2Wav`]).
pub trait SoundBank {
    /// Canonical archive path of the sound's file, or `None` if the id
    /// names no file.
    fn file(&self, id: SoundId) -> Option<Arc<str>>;
    /// Decoded samples of the sound's file, or `None` if it is missing.
    fn samples(&self, id: SoundId) -> Option<Arc<Sound>>;
    /// The file's size in bytes as the archive reports it (the
    /// uncompressed WAV), charged by the sample cache
    /// (`sound-table-2.md` §16 r5). `None` when the bank does not know it;
    /// the cache then charges the decoded data plus a 44-byte header.
    fn file_size(&self, _id: SoundId) -> Option<u64> {
        None
    }
}

/// What the voice policy decides for a start.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Admit {
    Start,
    /// Do not start (no voice, no log record).
    Reject,
    /// Stop this playing voice, then start.
    Steal(CueId),
}

/// Voice limits, stealing and repeat suppression (`audio.md` §A3).
///
/// The original's rules (`audio/sound-table.md` §6.2 r1, §7 r1, r3, r7:
/// 4 stereo and 12 mono channels, priority stealing, duplicate
/// suppression) are applied by [`sound_table::SoundSystem`] before a cue
/// is queued, so with it the core runs [`Unlimited`], which admits every
/// start.
pub trait VoicePolicy {
    fn admit(&mut self, trigger: &Trigger, playing: &[Voice]) -> Admit;
}

/// Admits every start: no limit, no stealing, no suppression.
#[derive(Clone, Copy, Debug, Default)]
pub struct Unlimited;

impl VoicePolicy for Unlimited {
    fn admit(&mut self, _: &Trigger, _: &[Voice]) -> Admit {
        Admit::Start
    }
}

/// Scheduler + mixer + voice log (`audio.md` §A3–§A5).
///
/// The game side calls [`pump`](Self::pump) / [`queue_mut`](Self::queue_mut)
/// and [`present`](Self::present) once per presented tick; the output side
/// calls [`mix_block`](Self::mix_block) whenever it needs samples. Cues
/// released by `present(t)` start at the next block.
pub struct AudioEngine {
    queue: TriggerQueue,
    presented: Option<u32>,
    released: Vec<(CueId, Cue)>,
    voices: Vec<Voice>,
    bank: Box<dyn SoundBank + Send>,
    gain: Box<dyn GainCurve + Send>,
    policy: Box<dyn VoicePolicy + Send>,
    log: VoiceLog,
    errors: Vec<AudioError>,
}

impl AudioEngine {
    pub fn new(
        bank: Box<dyn SoundBank + Send>,
        gain: Box<dyn GainCurve + Send>,
        policy: Box<dyn VoicePolicy + Send>,
    ) -> Self {
        AudioEngine {
            queue: TriggerQueue::new(),
            presented: None,
            released: Vec::new(),
            voices: Vec::new(),
            bank,
            gain,
            policy,
            log: VoiceLog::default(),
            errors: Vec::new(),
        }
    }

    pub fn queue_mut(&mut self) -> &mut TriggerQueue {
        &mut self.queue
    }

    /// Pull this frame's cues from the bridge seam.
    pub fn pump(&mut self, source: &mut dyn CueSource) {
        source.drain_cues(&mut self.queue);
    }

    /// The client presents tick `t`: every queued cue with `tick <= t` is
    /// released to start at the next block (`audio.md` §A3). Ticks never
    /// go back. A cue for an already presented tick is still released (its
    /// log record keeps its own tick) but is also reported as
    /// [`AudioError::LateCue`].
    pub fn present(&mut self, t: u32) -> Result<(), AudioError> {
        if let Some(previous) = self.presented {
            if t < previous {
                return Err(AudioError::TickBackwards { tick: t, previous });
            }
        }
        let due = self.queue.take_due(t);
        if let Some(presented) = self.presented {
            for (_, c) in &due {
                if c.tick() <= presented {
                    self.errors.push(AudioError::LateCue {
                        tick: c.tick(),
                        presented,
                    });
                }
            }
        }
        self.released.extend(due);
        self.presented = Some(t);
        Ok(())
    }

    /// Apply released cues in order, then mix one block (`audio.md` §A4).
    pub fn mix_block(&mut self) -> [i16; BLOCK_SAMPLES] {
        for (id, cue) in std::mem::take(&mut self.released) {
            match cue {
                Cue::Start(t) => self.start(id, t),
                Cue::Stop(s) => self.stop(s),
                Cue::Param(p) => self.param(p),
                Cue::Device(d) => self.device(d),
            }
        }
        mix(&mut self.voices)
    }

    pub fn voices(&self) -> &[Voice] {
        &self.voices
    }

    pub fn log(&self) -> &VoiceLog {
        &self.log
    }

    pub fn take_errors(&mut self) -> Vec<AudioError> {
        std::mem::take(&mut self.errors)
    }

    /// A failed start still appends a `Start` record with `error` set, so
    /// a log comparison shows it (`audio.md` Edge cases).
    fn start(&mut self, id: CueId, t: Trigger) {
        let record = |file: &str, error: bool| VoiceEvent {
            tick: t.tick,
            kind: VoiceKind::Start,
            file: file.to_owned(),
            vol: t.params.vol,
            pan: t.params.pan,
            looped: t.params.looped,
            error,
            cause: t.cause.clone(),
        };
        let Some(file) = self.bank.file(t.sound) else {
            self.log.push(record(&format!("#{}", t.sound.0), true));
            self.errors.push(AudioError::UnknownSound {
                sound: t.sound,
                cause: t.cause.clone(),
            });
            return;
        };
        let Some(sound) = self.bank.samples(t.sound) else {
            self.log.push(record(&file, true));
            self.errors.push(AudioError::MissingFile {
                file: file.to_string(),
                cause: t.cause.clone(),
            });
            return;
        };
        let gains = match self.gain.gains(t.params.vol, t.params.pan) {
            Ok(g) => g,
            Err(e) => {
                self.log.push(record(&file, true));
                self.errors.push(e);
                return;
            }
        };
        match self.policy.admit(&t, &self.voices) {
            Admit::Reject => return,
            Admit::Start => {}
            Admit::Steal(victim) => {
                if !self.voices.iter().any(|v| v.id() == victim) {
                    self.errors.push(AudioError::BadSteal(victim));
                    return;
                }
                self.stop(Stop {
                    tick: t.tick,
                    target: StopTarget::Voice(victim),
                    cause: format!("steal:{}", t.cause),
                });
            }
        }
        self.log.push(record(&file, false));
        self.voices
            .push(Voice::new(id, file, sound, t.params, gains));
    }

    /// Stopping a voice that is not playing (ended, never started) logs
    /// nothing. The sound layer sends a stop only for a channel it holds
    /// (`sound-table.md` §6.6 r1: the stop of §6.2 r2 and §6.3 r5, the
    /// steal of §7 r3; a fade to 0 of §5 r5 / r7 and the detach and group
    /// stops of `triggers.md` §1 r3–r4 reach a voice only through it), so
    /// a stop that finds no voice is one whose one-shot sample already
    /// ran out in the mixer, which the original's device also stops
    /// without a sound (§6.6 r4: silence past the end).
    fn stop(&mut self, s: Stop) {
        let mut stopped = Vec::new();
        self.voices.retain(|v| {
            let hit = match s.target {
                StopTarget::All => true,
                StopTarget::Voice(id) => v.id() == id,
            };
            if hit {
                stopped.push(v.clone());
            }
            !hit
        });
        for v in stopped {
            self.log.push(VoiceEvent {
                tick: s.tick,
                kind: VoiceKind::Stop,
                file: v.file().to_owned(),
                vol: v.params().vol,
                pan: v.params().pan,
                looped: v.params().looped,
                error: false,
                cause: s.cause.clone(),
            });
        }
    }

    /// `sound-table.md` §7 r4, r8, §8.3 r3: start frame, loop start and
    /// occlusion of a playing voice; its gains are recomputed. Not logged.
    fn device(&mut self, d: DeviceChange) {
        let Some(v) = self.voices.iter_mut().find(|v| v.id() == d.target) else {
            return;
        };
        v.set_device(d.start_frame, d.loop_start, f32::from_bits(d.occlusion));
        match self
            .gain
            .gains_occluded(v.params().vol, v.params().pan, v.occlusion())
        {
            Ok(g) => v.set_gains(g),
            Err(e) => self.errors.push(e),
        }
    }

    fn param(&mut self, p: ParamChange) {
        let Some(v) = self.voices.iter_mut().find(|v| v.id() == p.target) else {
            return;
        };
        match self.gain.gains_occluded(p.vol, p.pan, v.occlusion()) {
            Ok(g) => {
                v.set_params(p.vol, p.pan, g);
                self.log.push(VoiceEvent {
                    tick: p.tick,
                    kind: VoiceKind::Param,
                    file: v.file().to_owned(),
                    vol: p.vol,
                    pan: p.pan,
                    looped: v.params().looped,
                    error: false,
                    cause: p.cause,
                });
            }
            Err(e) => self.errors.push(e),
        }
    }
}
