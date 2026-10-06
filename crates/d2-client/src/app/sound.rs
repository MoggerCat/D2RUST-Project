// Spec: specs/client/audio.md (A1 decode path, A3 scheduler), specs/client/assets.md (A5 sounds)
//! The play mode's audio: the audio core ([`AudioEngine`]) reading its
//! samples through the sound pool ([`SoundPool`], `audio.md` §A1: each
//! file read and decoded once, kept under the `sounds` budget), driven
//! once per Bevy frame after the bridge frame (`bridge.md` §8): the pool
//! starts the frame, the cue source's cues are queued, and the frame's
//! server tick is presented (`audio.md` §A3). The output edge
//! ([`crate::audio::output`]) mixes on the audio thread when the app has
//! an audio device.
//!
//! Plumbing only. Every original rule is a hook with the narrowest
//! answer: which events make sounds ([`CueSource`], [`NoCues`]:
//! TODO(spec: audio/triggers.md)), sound ids to files ([`SoundTable`],
//! [`NoSoundTable`]: TODO(spec: audio/sound-table.md)), the WAV decode
//! ([`WavDecoder`], [`NoWavDecoder`]: TODO(spec: formats/wav.md §B1)),
//! gain and voice policy (the core's placeholders). With those, the app
//! starts no voice.

use std::sync::{Arc, Mutex};

use bevy::prelude::*;

use crate::assets::cache::{Budgets, CacheEvent};
use crate::assets::path::{CanonicalPath, FileSource, MemorySource};
use crate::audio::output::{self, MixerStream};
use crate::audio::{
    AudioEngine, AudioError, CueSource, GainCurve, Sound, SoundBank, SoundId, SoundPool,
    SoundPoolError, TriggerQueue, UnityGain, Unlimited, VoicePolicy, WavDecoder,
};
use crate::bridge::BridgeResource;

/// Sound ids to archive paths.
///
/// TODO(spec: audio/sound-table.md) (`audio.md` §B3): the `sounds.txt`
/// mapping and its variants.
pub trait SoundTable: Send + Sync {
    fn file(&self, id: SoundId) -> Option<Arc<str>>;
}

/// No sound table yet: no id names a file.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoSoundTable;

impl SoundTable for NoSoundTable {
    fn file(&self, _: SoundId) -> Option<Arc<str>> {
        None
    }
}

/// No WAV decoder yet: every decode is an error naming the spec.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoWavDecoder;

impl WavDecoder for NoWavDecoder {
    fn decode(&self, _: &CanonicalPath, _: &[u8]) -> Result<Sound, String> {
        Err("no WAV decoder: TODO(spec: formats/wav.md §B1)".into())
    }
}

/// No cue source yet: no event makes a sound.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoCues;

impl CueSource for NoCues {
    fn drain_cues(&mut self, _: &mut TriggerQueue) {}
}

/// The engine's [`SoundBank`]: the table's file, loaded through the pool.
/// A failed load is kept for the frame's report and answers `None` (the
/// engine logs the start as failed, `audio.md` Edge cases).
pub struct PoolBank {
    table: Box<dyn SoundTable>,
    pool: Arc<Mutex<SoundPool>>,
    errors: Arc<Mutex<Vec<SoundPoolError>>>,
}

impl SoundBank for PoolBank {
    fn file(&self, id: SoundId) -> Option<Arc<str>> {
        self.table.file(id)
    }

    fn samples(&self, id: SoundId) -> Option<Arc<Sound>> {
        let file = self.table.file(id)?;
        let loaded = self.pool.lock().ok()?.load(&file);
        match loaded {
            Ok(sound) => Some(sound),
            Err(e) => {
                if let Ok(mut errors) = self.errors.lock() {
                    errors.push(e);
                }
                None
            }
        }
    }
}

/// The parts of the play mode's audio; each hook defaults to its
/// placeholder.
pub struct AudioParts {
    pub source: Arc<dyn FileSource>,
    pub decoder: Box<dyn WavDecoder>,
    pub table: Box<dyn SoundTable>,
    pub cues: Box<dyn CueSource + Send + Sync>,
    pub gain: Box<dyn GainCurve + Send>,
    pub policy: Box<dyn VoicePolicy + Send>,
    /// The `sounds` pool budget (`assets.md` §A5).
    pub budget: u64,
}

impl AudioParts {
    /// Every hook at its placeholder over `source` (the user's archives,
    /// or an empty source without game files).
    pub fn unspecified(source: Arc<dyn FileSource>) -> Self {
        AudioParts {
            source,
            decoder: Box::new(NoWavDecoder),
            table: Box::new(NoSoundTable),
            cues: Box::new(NoCues),
            gain: Box::new(UnityGain),
            policy: Box::new(Unlimited),
            budget: Budgets::default().sounds,
        }
    }

    /// [`AudioParts::unspecified`] over an empty source.
    pub fn empty() -> Self {
        Self::unspecified(Arc::new(MemorySource::default()))
    }
}

/// What the last audio frame did, for logs and tests.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AudioStats {
    /// Bridge frame of the last pool frame.
    pub frame: u64,
    /// Server tick presented last.
    pub presented: u32,
    /// Files decoded so far.
    pub decodes: u64,
    /// Sound loads that failed, and engine errors, so far.
    pub load_errors: usize,
    pub engine_errors: usize,
}

/// The play mode's audio state.
#[derive(Resource)]
pub struct GameAudio {
    /// Shared with the output edge, which mixes from it.
    pub engine: Arc<Mutex<AudioEngine>>,
    pub pool: Arc<Mutex<SoundPool>>,
    cues: Box<dyn CueSource + Send + Sync>,
    errors: Arc<Mutex<Vec<SoundPoolError>>>,
    pub stats: AudioStats,
}

impl GameAudio {
    pub fn new(parts: AudioParts) -> Self {
        let pool = Arc::new(Mutex::new(SoundPool::new(
            parts.source,
            parts.decoder,
            parts.budget,
        )));
        let errors = Arc::new(Mutex::new(Vec::new()));
        let bank = PoolBank {
            table: parts.table,
            pool: Arc::clone(&pool),
            errors: Arc::clone(&errors),
        };
        GameAudio {
            engine: Arc::new(Mutex::new(AudioEngine::new(
                Box::new(bank),
                parts.gain,
                parts.policy,
            ))),
            pool,
            cues: parts.cues,
            errors,
            stats: AudioStats::default(),
        }
    }
}

/// Errors of an audio frame (Bevy's error handler reports them).
#[derive(Debug, thiserror::Error)]
pub enum AudioFrameError {
    #[error("audio state poisoned (the audio thread panicked)")]
    Poisoned,
    #[error("server tick {0} does not fit the audio tick")]
    Tick(u64),
    #[error(transparent)]
    Cache(#[from] crate::assets::cache::CacheError),
    #[error(transparent)]
    Audio(#[from] AudioError),
}

/// Adds the audio frame (after the bridge frame) and a [`GameAudio`] with
/// `parts`. A later `insert_resource(GameAudio::new(…))` replaces it.
pub fn add_audio(app: &mut App, parts: AudioParts) {
    app.insert_resource(GameAudio::new(parts)).add_systems(
        Update,
        audio_frame
            .run_if(resource_exists::<BridgeResource>)
            .run_if(resource_exists::<GameAudio>),
    );
}

/// Plays the mixed stream on the app's audio device. Needs Bevy's audio
/// plugin (the window's `DefaultPlugins`).
pub fn add_output(app: &mut App) {
    output::register(app);
    app.add_systems(PostStartup, start_output);
}

fn start_output(
    mut commands: Commands,
    audio: Option<Res<GameAudio>>,
    mut streams: ResMut<Assets<MixerStream>>,
) {
    if let Some(audio) = audio {
        let stream = streams.add(MixerStream::new(Arc::clone(&audio.engine)));
        commands.spawn(AudioPlayer(stream));
    }
}

/// One audio frame: pool frame, cues, present the frame's server tick.
fn audio_frame(
    bridge: Res<BridgeResource>,
    mut audio: ResMut<GameAudio>,
) -> std::result::Result<(), BevyError> {
    let audio = &mut *audio;
    let world = bridge.0.world();
    let tick =
        u32::try_from(world.server_ticks).map_err(|_| AudioFrameError::Tick(world.server_ticks))?;
    let decodes = {
        let mut pool = audio.pool.lock().map_err(|_| AudioFrameError::Poisoned)?;
        pool.begin_frame(world.frames)
            .map_err(AudioFrameError::from)?;
        for e in pool.drain_events() {
            if let CacheEvent::Evicted { key, .. } = &e {
                debug!("sound pool evicted {key}");
            } else {
                warn!("sound pool: {e:?}");
            }
        }
        pool.decodes()
    };
    let engine_errors = {
        let mut engine = audio.engine.lock().map_err(|_| AudioFrameError::Poisoned)?;
        engine.pump(audio.cues.as_mut());
        engine.present(tick).map_err(AudioFrameError::from)?;
        engine.take_errors()
    };
    for e in &engine_errors {
        warn!("audio: {e}");
    }
    let load_errors =
        std::mem::take(&mut *audio.errors.lock().map_err(|_| AudioFrameError::Poisoned)?);
    for e in &load_errors {
        warn!("sound load: {e}");
    }
    let s = &mut audio.stats;
    s.frame = world.frames;
    s.presented = tick;
    s.decodes = decodes;
    s.load_errors += load_errors.len();
    s.engine_errors += engine_errors.len();
    Ok(())
}
