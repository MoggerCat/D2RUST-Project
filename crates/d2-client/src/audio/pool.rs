// Spec: specs/client/audio.md
// Spec: specs/client/assets.md (§A5 sound pool budget)
//! Sound pool (`audio.md` §A1 decode path, `assets.md` §A5 pool
//! `sounds`): each `.wav` is read once from the archive set
//! ([`FileSource`], `ArchiveSet` in the client), decoded once by a
//! [`WavDecoder`], and kept as an `Arc<Sound>` keyed by its canonical path.
//! The decoder's samples are stored as given: no resampling, no float.
//! Residency follows the shared [`Pool`] rules (budget in bytes =
//! samples × 2, LRU by last frame used, current frame never evicted).
//!
//! The RIFF/WAVE parse itself is original behavior:
//! TODO(spec: formats/wav.md §B1). Nothing here implements [`WavDecoder`];
//! the client gets one when `d2-formats::wav` exists.

use std::sync::Arc;

use crate::assets::cache::{CacheError, CacheEvent, FrameNo, Pool};
use crate::assets::path::{read_asset, CanonicalPath, FileSource, PathError, ReadError};

use super::Sound;

/// Pool name in cache events.
pub const POOL_NAME: &str = "sounds";

/// RIFF bytes → [`Sound`].
///
/// TODO(spec: formats/wav.md §B1): the WAV subset of the 1.14d archives
/// and the samples 1.14d hands to its sound output.
pub trait WavDecoder: Send + Sync {
    fn decode(&self, path: &CanonicalPath, bytes: &[u8]) -> Result<Sound, String>;
}

/// A failed sound load. Every variant names the path (`assets.md` §A1: no
/// fallback sound).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SoundPoolError {
    #[error(transparent)]
    Path(#[from] PathError),
    #[error(transparent)]
    Read(#[from] ReadError),
    #[error("sound {path}: decode failed: {message}")]
    Decode {
        path: CanonicalPath,
        message: String,
    },
    #[error(transparent)]
    Cache(#[from] CacheError),
}

/// Bytes a sound is charged in the pool (`assets.md` §A5: samples × 2).
pub fn sound_bytes(sound: &Sound) -> u64 {
    sound.samples().len() as u64 * 2
}

/// Decoded sounds by canonical path, under the `sounds` budget.
pub struct SoundPool {
    source: Arc<dyn FileSource>,
    decoder: Box<dyn WavDecoder>,
    pool: Pool<CanonicalPath, Arc<Sound>>,
    decodes: u64,
}

impl SoundPool {
    pub fn new(source: Arc<dyn FileSource>, decoder: Box<dyn WavDecoder>, budget: u64) -> Self {
        SoundPool {
            source,
            decoder,
            pool: Pool::new(POOL_NAME, budget),
            decodes: 0,
        }
    }

    /// Starts frame `frame` of the pool (see [`Pool::begin_frame`]).
    pub fn begin_frame(&mut self, frame: FrameNo) -> Result<(), CacheError> {
        self.pool.begin_frame(frame)
    }

    /// The sound of the archive path `path` (any spelling: canonicalized
    /// first, so two spellings share one entry). Resident: returned and
    /// marked used. Otherwise read, decoded and inserted once.
    pub fn load(&mut self, path: &str) -> Result<Arc<Sound>, SoundPoolError> {
        let path = CanonicalPath::new(path)?;
        if let Some(sound) = self.pool.get(&path) {
            return Ok(Arc::clone(sound));
        }
        let bytes = read_asset(self.source.as_ref(), path.as_str())?;
        self.decodes += 1;
        let sound =
            self.decoder
                .decode(&path, &bytes)
                .map_err(|message| SoundPoolError::Decode {
                    path: path.clone(),
                    message,
                })?;
        // `Sound` is only built through the strict `Sound::new`.
        let sound = Arc::new(sound);
        let bytes = sound_bytes(&sound);
        self.pool.insert(path, Arc::clone(&sound), bytes)?;
        Ok(sound)
    }

    /// The resident sound of `path` without marking it used or loading.
    pub fn peek(&self, path: &str) -> Option<Arc<Sound>> {
        let path = CanonicalPath::new(path).ok()?;
        self.pool.peek(&path).cloned()
    }

    /// Files decoded so far (a resident hit does not count).
    pub fn decodes(&self) -> u64 {
        self.decodes
    }

    /// Bytes charged by resident sounds.
    pub fn used(&self) -> u64 {
        self.pool.used()
    }

    pub fn len(&self) -> usize {
        self.pool.len()
    }

    pub fn is_empty(&self) -> bool {
        self.pool.is_empty()
    }

    /// Takes the pool's events (evictions, overruns), oldest first.
    pub fn drain_events(&mut self) -> Vec<CacheEvent> {
        self.pool.drain_events()
    }
}
