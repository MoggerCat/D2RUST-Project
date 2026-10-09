// Spec: specs/client/audio.md (A1 decode path, A3 scheduler), specs/client/assets.md (A5 sounds)
// Spec: specs/audio/sound-table.md (§1–§7, §13 sound table hooks), specs/audio/sound-table-2.md (§16–§17)
// Spec: specs/audio/triggers.md, specs/audio/triggers-2.md (§21 driver inputs)
// Spec: specs/formats/wav.md (§5 decoder hook)
//! The play mode's audio: the audio core ([`AudioEngine`]) reading its
//! samples through the sound pool ([`SoundPool`], `audio.md` §A1: each
//! file read and decoded once, kept under the `sounds` budget), driven
//! once per Bevy frame after the bridge frame (`bridge.md` §8): the pool
//! starts the frame, the cue source's cues are queued, and the frame's
//! server tick is presented (`audio.md` §A3). The output edge
//! ([`crate::audio::output`]) mixes on the audio thread when the app has
//! an audio device.
//!
//! Plumbing only; the original rules sit behind hooks. With a sound table
//! ([`AudioParts::original`]) the cues come from the sound layer
//! ([`SoundDriver`]: requests per `audio/triggers.md` with the inputs of
//! `audio/triggers-2.md` §21; channels, stealing and volume / pan per
//! `audio/sound-table.md` §5–§8), sound ids map to files by
//! `sound-table.md` §1, §3, §4 ([`SoundTable`]), the WAV decode is
//! `formats/wav.md` §5 ([`D2Wav`]) and the gain curve is §8.3
//! ([`DeviceGain`]). [`AudioParts::unspecified`] keeps every hook at its
//! neutral implementation ([`NoCues`], [`NoSoundTable`], [`NoWavDecoder`],
//! [`UnityGain`], [`Unlimited`]): the app then starts no voice.

use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use d2_data::bin::TableFiles;
use d2_data::txt::TxtTable;

use crate::app::config::{ConfigRes, Settings};
use crate::assets::cache::{Budgets, CacheEvent};
use crate::assets::path::{CanonicalPath, FileSource, MemorySource};
use crate::audio::driver::{DriverError, SoundDriver, SoundLink};
use crate::audio::output::{self, MixerStream};
use crate::audio::sound_table::{DeviceGain, SoundSettings, SoundSystem, SoundTableData};
use crate::audio::D2Wav;
use crate::audio::{
    AudioEngine, AudioError, CueSource, GainCurve, Sound, SoundBank, SoundId, SoundPool,
    SoundPoolError, TriggerQueue, UnityGain, Unlimited, VoicePolicy, WavDecoder,
};
use crate::bridge::BridgeResource;
use crate::world_view::walk::PreviewWalk;
use crate::world_view::UiSounds;

/// Sound ids to archive paths: the `sounds.txt` mapping of
/// `audio/sound-table.md` §1 (id = data-line index) and §3 (path prefix by
/// id range, then `FileName`); variants are picked by the sound layer (§4
/// r3) before an id reaches this hook. The implementation is
/// [`crate::audio::sound_table::SoundPaths`].
pub trait SoundTable: Send + Sync {
    fn file(&self, id: SoundId) -> Option<Arc<str>>;
}

/// No sound table (the hook-only play mode): no id names a file.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoSoundTable;

impl SoundTable for NoSoundTable {
    fn file(&self, _: SoundId) -> Option<Arc<str>> {
        None
    }
}

/// No WAV decoder (the hook-only play mode): every decode is an error. The
/// decoder of `formats/wav.md` §5 is [`D2Wav`].
#[derive(Debug, Clone, Copy, Default)]
pub struct NoWavDecoder;

impl WavDecoder for NoWavDecoder {
    fn decode(&self, _: &CanonicalPath, _: &[u8]) -> Result<Sound, String> {
        Err("no WAV decoder in this play mode (formats/wav.md §5 is D2Wav)".into())
    }
}

/// No cue source (the hook-only play mode): no event makes a sound.
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

    /// The file's byte size as read, charged by the sample cache
    /// (`sound-table-2.md` §16 r5, `formats/wav.md` §5).
    fn file_size(&self, id: SoundId) -> Option<u64> {
        let file = self.table.file(id)?;
        self.pool.lock().ok()?.file_size(&file)
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

/// The parts of the play mode's audio; each hook defaults to its neutral
/// implementation.
pub struct AudioParts {
    pub source: Arc<dyn FileSource>,
    pub decoder: Box<dyn WavDecoder>,
    pub table: Box<dyn SoundTable>,
    pub cues: Box<dyn CueSource + Send + Sync>,
    pub gain: Box<dyn GainCurve + Send>,
    pub policy: Box<dyn VoicePolicy + Send>,
    /// The `sounds` pool budget (`assets.md` §A5).
    pub budget: u64,
    /// The sound table (`sound-table.md`): with it the cues come from the
    /// original sound layer ([`SoundDriver`]) instead of `cues`.
    pub sounds: Option<SoundTableData>,
}

impl AudioParts {
    /// Every hook at its neutral implementation over `source` (the user's archives,
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
            sounds: None,
        }
    }

    /// The original audio over `source` (`client/audio.md` §B1–§B3): the
    /// sound table's paths, the 1.14d WAV decoder ([`D2Wav`]), the sound
    /// layer as the cue source (channels and stealing are its own, so the
    /// core admits every start, [`Unlimited`]) and the device gain curve
    /// ([`DeviceGain`], `sound-table.md` §8.3).
    pub fn original(source: Arc<dyn FileSource>, table: SoundTableData) -> Self {
        AudioParts {
            source,
            decoder: Box::new(D2Wav),
            table: Box::new(table.paths()),
            cues: Box::new(NoCues),
            gain: Box::new(DeviceGain),
            policy: Box::new(Unlimited),
            budget: Budgets::default().sounds,
            sounds: Some(table),
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
    /// Tick presented last (the sound tick when the sound layer runs,
    /// else the server tick).
    pub presented: u32,
    /// Files decoded so far.
    pub decodes: u64,
    /// Sound loads that failed, and engine errors, so far.
    pub load_errors: usize,
    pub engine_errors: usize,
    /// The sound layer's pending model questions seen so far, each once,
    /// in first-seen order (`SoundDriver::take_pending`).
    pub pending: Vec<&'static str>,
}

/// The play mode's audio state.
#[derive(Resource)]
pub struct GameAudio {
    /// Shared with the output edge, which mixes from it.
    pub engine: Arc<Mutex<AudioEngine>>,
    pub pool: Arc<Mutex<SoundPool>>,
    cues: Box<dyn CueSource + Send + Sync>,
    /// The original sound layer, when the parts carry a sound table
    /// (shared with the thunder step through [`SoundLink`]).
    pub driver: Option<Arc<Mutex<SoundDriver>>>,
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
        let driver = parts.sounds.map(|table| {
            let paths = table.paths();
            let bank = PoolBank {
                table: Box::new(paths),
                pool: Arc::clone(&pool),
                errors: Arc::clone(&errors),
            };
            Arc::new(Mutex::new(SoundDriver::new(SoundSystem::new(
                table,
                Box::new(bank),
            ))))
        });
        GameAudio {
            engine: Arc::new(Mutex::new(AudioEngine::new(
                Box::new(bank),
                parts.gain,
                parts.policy,
            ))),
            pool,
            cues: parts.cues,
            driver,
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
    #[error(transparent)]
    Driver(#[from] DriverError),
}

/// Adds the audio frame (after the bridge frame) and a [`GameAudio`] with
/// `parts`. A later `insert_resource(GameAudio::new(…))` replaces it.
pub fn add_audio(app: &mut App, parts: AudioParts) {
    app.insert_resource(GameAudio::new(parts))
        .init_resource::<SoundLink>()
        .add_systems(
            PostUpdate,
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

/// `play --sound-log FILE` (`specs/tools/facts-render.md` §5 r20): every
/// sound request call, one row each, tagged with the server tick of the
/// audio frame that made it.
#[derive(Resource)]
pub struct SoundLog {
    out: std::io::BufWriter<std::fs::File>,
}

/// The log's header lines (format version 1).
pub const SOUND_LOG_HEADER: &str =
    "# sound-log v1; d2-client play --sound-log\ntick\tsound_tick\tid\tunit_type\tguid\tdelay\tflags\toffset\n";

impl SoundLog {
    pub fn create(path: &std::path::Path) -> std::io::Result<Self> {
        use std::io::Write;
        let mut out = std::io::BufWriter::new(std::fs::File::create(path)?);
        out.write_all(SOUND_LOG_HEADER.as_bytes())?;
        out.flush()?;
        Ok(SoundLog { out })
    }

    /// One row per call; `-` for no unit.
    pub fn write(
        &mut self,
        tick: u64,
        calls: &[crate::audio::sound_table::system::RequestCall],
    ) -> std::io::Result<()> {
        use std::io::Write;
        for c in calls {
            let (t, g) = c.unit.map_or(("-".to_owned(), "-".to_owned()), |u| {
                (u.unit_type.to_string(), u.guid.to_string())
            });
            writeln!(
                self.out,
                "{tick}\t{}\t{}\t{t}\t{g}\t{}\t{}\t{}",
                c.sound_tick, c.id, c.delay, c.flags, c.offset
            )?;
        }
        self.out.flush()
    }
}

/// One audio frame: pool frame, the sound layer's ticks (when it runs:
/// the UI's sound requests first), cues, present the tick (the sound
/// tick with the sound layer, else the frame's server tick).
fn audio_frame(
    bridge: Res<BridgeResource>,
    mut audio: ResMut<GameAudio>,
    ui_sounds: Option<ResMut<UiSounds>>,
    walk: Option<Res<PreviewWalk>>,
    config: Option<Res<ConfigRes>>,
    link: Option<Res<SoundLink>>,
    mut log: Option<ResMut<SoundLog>>,
) -> std::result::Result<(), BevyError> {
    let audio = &mut *audio;
    let weather = link.as_deref().map(|l| {
        l.set(audio.driver.as_ref());
        l.weather()
    });
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
    let requests = ui_sounds.map(|mut s| std::mem::take(&mut s.0));
    let mut driver = match &audio.driver {
        Some(d) => Some(d.lock().map_err(|_| AudioFrameError::Poisoned)?),
        None => None,
    };
    if let Some(d) = driver.as_deref_mut() {
        // The options the menu wrote (`sound-table.md` §9; heard from the
        // next sound tick).
        if let Some(c) = config.as_deref() {
            let s = sound_settings(&c.settings, *d.system().settings());
            d.set_settings(s);
        }
        if let Some(w) = weather {
            d.set_weather(w);
        }
        // The listener is where the player is drawn (`seams/bridge-app.md`
        // §2.7).
        d.set_local_prediction(walk.as_deref().and_then(PreviewWalk::local_at));
        d.set_local_mode(
            walk.as_deref()
                .and_then(|w| Some((w.predict.player()?, w.predict.mode()?))),
        );
        if log.is_some() {
            d.log_requests(true);
        }
        d.frame(
            world,
            &bridge.0.inputs().tables.levels,
            requests.as_deref().unwrap_or_default(),
        )
        .map_err(AudioFrameError::from)?;
        if let Some(l) = log.as_deref_mut() {
            if let Err(e) = l.write(world.server_ticks, &d.take_request_log()) {
                warn!("sound log: {e}");
            }
        }
        // A model question the sound layer could not answer is not an
        // error (`seams/bridge-app.md` §2.9): answered neutrally, logged
        // once per question.
        for q in d.take_pending() {
            if !audio.stats.pending.contains(&q) {
                warn!("sound layer pending: {q}");
                audio.stats.pending.push(q);
            }
        }
        for s in d.take_skipped() {
            debug!("sound layer skipped: {s}");
        }
        for e in d.take_errors() {
            warn!("sound layer: {e}");
        }
    }
    let (presented, engine_errors) = {
        let mut engine = audio.engine.lock().map_err(|_| AudioFrameError::Poisoned)?;
        let presented = match driver.as_deref_mut() {
            Some(d) => {
                engine.pump(d);
                d.tick()
            }
            None => {
                engine.pump(audio.cues.as_mut());
                tick
            }
        };
        engine.present(presented).map_err(AudioFrameError::from)?;
        (presented, engine.take_errors())
    };
    drop(driver);
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
    s.presented = presented;
    s.decodes = decodes;
    s.load_errors += load_errors.len();
    s.engine_errors += engine_errors.len();
    Ok(())
}

/// The sound layer's settings from the app's (`sound-table.md` §9;
/// `frontend-options.md` §O7 keeps the 1.14d integers): mixer mode 0 (the
/// only one d2rs reproduces, §9), `Master Volume`, `Music Volume`,
/// `Positional Bias`, `NPC Speech`; `Options Music` and the game-loaded
/// flag are not app settings and keep `prev`'s.
pub fn sound_settings(s: &Settings, prev: SoundSettings) -> SoundSettings {
    SoundSettings {
        mixer_mode: 0,
        master_volume: i32::from(s.master_volume),
        music_volume: i32::from(s.music_volume),
        positional_bias: i32::from(s.positional_bias),
        npc_speech: i32::from(s.npc_speech),
        ..prev
    }
}

/// The user's `sounds.txt` and `soundenviron.txt` (`sound-table.md` §1,
/// §2), read from the archives and compiled; a missing or bad table is an
/// error, never a fallback.
pub fn sound_table_live(archives: &dyn TableFiles) -> Result<SoundTableData, String> {
    let txt = |f: &str| -> Result<TxtTable, String> {
        let (_, b) = archives
            .read_excel(f)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("{f}: in no archive"))?;
        TxtTable::parse(f, &b).map_err(|e| e.to_string())
    };
    SoundTableData::from_txt(&txt("sounds.txt")?, &txt("soundenviron.txt")?)
        .map_err(|e| e.to_string())
}
