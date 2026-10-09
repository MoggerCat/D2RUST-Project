// Spec: specs/audio/sound-table.md (§1 loading, §3 file path, §4 groups and variants, §10 r4 locks)
//! The loaded sound table: `sounds.txt` rows (compiled by
//! [`d2_data::sounds`]) plus the runtime fields of each record
//! (`sound-table.md` §1 r3: group base, block count, variant history,
//! sample state), the song range read from `soundenviron.txt` (§1 r5),
//! the archive path rule (§3) and the variant picker (§4 r3).

use std::ops::RangeInclusive;
use std::sync::Arc;

use d2_data::sounds::{compile_soundenviron, compile_sounds, SoundEnvironRow, SoundRow};
use d2_data::txt::{TxtError, TxtTable};

use crate::audio::{Sound, SoundId};

/// Speech ids: `DATA\LOCAL` prefix (§3 r1), speech instance rule (§6.3
/// r3), speech stops (`triggers.md` §1 r4).
pub const SPEECH: RangeInclusive<i32> = 2934..=4656;
/// Ids whose path takes `\MUSIC\` (§3 r2); also exempt from the state
/// duck (§6.5 r2).
pub const MUSIC_PATH: RangeInclusive<i32> = 4657..=4698;

/// Whether `id` is a speech id (§3 r1: `id − 0xB76 < 0x6BB`, unsigned).
pub fn is_speech(id: i32) -> bool {
    (id.wrapping_sub(0xB76) as u32) < 0x6BB
}

/// Whether `id` takes the `\MUSIC\` path (§3 r2: `id − 0x1231 < 0x2A`).
pub fn is_music_path(id: i32) -> bool {
    (id.wrapping_sub(0x1231) as u32) < 0x2A
}

/// A load or path error of the table.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SoundTableError {
    #[error(transparent)]
    Txt(#[from] TxtError),
    /// §3 r4: a path of 0x50 bytes or more (fatal `0x3AB` in 1.14d).
    #[error("sound {id}: path of {len} bytes (limit 79)")]
    PathTooLong { id: i32, len: usize },
    /// §10 r4: unlocking a zero lock count (fatal `0x1DC` in 1.14d).
    #[error("sound {id}: unlock of a zero lock count")]
    Unlock { id: i32 },
}

/// Sample state of a record (+0x86, §1 r3, §10).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum LoadState {
    #[default]
    None,
    /// Async load started, not collected yet.
    Pending,
    Loaded,
}

/// One record: its cells and its runtime fields (§1 r3).
#[derive(Clone, Debug)]
pub struct SoundEntry {
    pub row: SoundRow,
    /// +0x60 (§4 r1).
    pub group_base: i32,
    /// `Group Size` after the group pass (§4 r1): 0 stays 0 only on row 0.
    pub group_size: u8,
    /// +0x80 (§4 r2).
    pub block_count: u8,
    /// +0x64, +0x68: the last two variants chosen through this id, newest
    /// first (§4 r4).
    pub history: [i32; 2],
    /// +0x86.
    pub load: LoadState,
    /// +0x81: the file failed; never retried (§10 r2).
    pub failed: bool,
    /// +0x7C: sound tick of last use.
    pub last_use: u32,
    /// +0x70: sample size in bytes charged to the cache (`sound-table-2.md`
    /// §16 r2).
    pub size: u32,
    /// +0x8A (§10 r4).
    pub locks: u32,
    /// The decoded sample once loaded (d2rs: decoded up front by the bank,
    /// `client/assets.md` §A5; the state above decides availability).
    pub sample: Option<Arc<Sound>>,
}

/// `sounds.txt` with its runtime fields, and the song range (§1).
#[derive(Clone, Debug)]
pub struct SoundTableData {
    entries: Vec<SoundEntry>,
    song_range: Option<(i32, i32)>,
    /// `Indoors` (+0x18) of each `soundenviron` record (§2), the input of
    /// the thunder occlusion (§6.4 r2).
    indoors: Vec<u8>,
    /// The `soundenviron` records (§2), by row.
    environ: Vec<SoundEnvironRow>,
}

impl SoundTableData {
    /// Builds the table from compiled rows (row `i` = sound id `i`, §1 r1)
    /// and runs the group pass (§4 r1–r2) and the song range scan (§1 r5).
    pub fn new(rows: Vec<SoundRow>, environ: &[SoundEnvironRow]) -> Self {
        let mut entries: Vec<SoundEntry> = rows
            .into_iter()
            .enumerate()
            .map(|(i, row)| SoundEntry {
                group_base: i as i32,
                group_size: row.group_size,
                block_count: 0,
                history: [0, 0],
                load: LoadState::None,
                failed: false,
                last_use: 0,
                size: 0,
                locks: 0,
                sample: None,
                row,
            })
            .collect();
        // §4 r1–r2: ids 1..count−1; row 0 keeps base 0 and its size.
        let mut group: Option<(usize, usize)> = None;
        for (i, e) in entries.iter_mut().enumerate().skip(1) {
            e.group_base = match group {
                Some((s, g)) if s <= i && i < s + g => s as i32,
                _ => i as i32,
            };
            if e.row.group_size != 0 {
                group = Some((i, usize::from(e.row.group_size)));
            } else {
                e.group_size = 1;
            }
            e.block_count = e.row.blocks.iter().take_while(|&&b| b != -1).count() as u8;
        }
        // §1 r5: minimum and maximum `Song` over rows with `Song > 0`.
        let songs = environ.iter().map(|r| r.song).filter(|&s| s > 0);
        let song_range = songs
            .clone()
            .min()
            .and_then(|lo| songs.max().map(|hi| (lo, hi)));
        SoundTableData {
            entries,
            song_range,
            indoors: environ.iter().map(|r| r.indoors).collect(),
            environ: environ.to_vec(),
        }
    }

    /// The `soundenviron` records, by row (§2).
    pub fn env_rows(&self) -> &[SoundEnvironRow] {
        &self.environ
    }

    /// `Indoors` of each `soundenviron` record, by row (§2).
    pub fn env_indoors(&self) -> &[u8] {
        &self.indoors
    }

    /// Compiles `sounds.txt` and `soundenviron.txt` (§1, §2) with the
    /// `.txt` reader and builds the table.
    pub fn from_txt(sounds: &TxtTable, environ: &TxtTable) -> Result<Self, SoundTableError> {
        let rows = compile_sounds("DATA\\GLOBAL\\EXCEL\\sounds.txt", sounds)?;
        let env = compile_soundenviron("DATA\\GLOBAL\\EXCEL\\soundenviron.txt", environ)?;
        Ok(Self::new(rows.rows, &env.rows))
    }

    /// Record count (= data lines).
    pub fn count(&self) -> usize {
        self.entries.len()
    }

    /// Record access (`0x00481860`, §1 r4): nothing outside `0..count−1`.
    pub fn get(&self, id: i32) -> Option<&SoundEntry> {
        usize::try_from(id).ok().and_then(|i| self.entries.get(i))
    }

    pub fn get_mut(&mut self, id: i32) -> Option<&mut SoundEntry> {
        usize::try_from(id)
            .ok()
            .and_then(|i| self.entries.get_mut(i))
    }

    /// The group base of `id` (+0x60), or `id` itself outside the table.
    pub fn base(&self, id: i32) -> i32 {
        self.get(id).map_or(id, |e| e.group_base)
    }

    /// The song range (§1 r5), if any row has `Song > 0`.
    pub fn song_range(&self) -> Option<(i32, i32)> {
        self.song_range
    }

    pub fn is_song(&self, id: i32) -> bool {
        self.song_range.is_some_and(|(lo, hi)| lo <= id && id <= hi)
    }

    /// The archive path of `id` (`0x00482710`, §3), `None` outside the
    /// table. Ranges are fixed in code, not taken from the data.
    pub fn path(&self, id: i32) -> Option<Result<String, SoundTableError>> {
        let e = self.get(id)?;
        let mut p = String::from(if is_speech(id) {
            "DATA\\LOCAL"
        } else {
            "DATA\\GLOBAL"
        });
        p.push_str(if is_music_path(id) {
            "\\MUSIC\\"
        } else {
            "\\SFX\\"
        });
        // `FileName` as written; cells are bytes, live names are ASCII.
        p.extend(e.row.file_name.iter().map(|&b| char::from(b)));
        Some(if p.len() >= 0x50 {
            Err(SoundTableError::PathTooLong { id, len: p.len() })
        } else {
            Ok(p)
        })
    }

    /// Variant pick (`0x00482680`, §4 r3). `roll` is the client RNG of §4
    /// r5. The variant is relative to `id`, not to its group base.
    pub fn pick_variant(&self, id: i32, roll: &mut dyn FnMut(i32) -> u32) -> i32 {
        let Some(e) = self.get(id) else {
            return id;
        };
        let n = i32::from(e.group_size);
        if n <= 1 {
            return id;
        }
        let mut k = (n - 1).min(2) as usize;
        if n <= 3 && roll(n + 1) == 0 {
            k -= 1;
        }
        // Bounded: with a real seed a miss has probability <= 1/2 per draw, so
        // 64 misses never happen; a roll stuck at 0 (no client seed yet,
        // `SoundSystem::roll`) would otherwise spin forever.
        for _ in 0..64 {
            let v = id.wrapping_add(roll(n) as i32);
            if !e.history[..k].contains(&v) {
                return v;
            }
        }
        id
    }

    /// History update (`0x004E0213`, §4 r4) on the requested id's record.
    pub fn record_history(&mut self, requested: i32, chosen: i32) {
        if let Some(e) = self.get_mut(requested) {
            e.history = [chosen, e.history[0]];
        }
    }

    /// `0x004822B0(id, ±1)` over the id's whole group (§10 r4): the ids
    /// `base .. base + size` of `id`'s group base.
    pub fn lock(&mut self, id: i32, delta: i32) -> Result<(), SoundTableError> {
        let base = self.base(id);
        let size = self.get(base).map_or(1, |e| i32::from(e.group_size).max(1));
        for g in base..base.saturating_add(size) {
            let Some(e) = self.get_mut(g) else { continue };
            if delta < 0 {
                if e.locks == 0 {
                    return Err(SoundTableError::Unlock { id: g });
                }
                e.locks -= 1;
            } else {
                e.locks += 1;
            }
        }
        Ok(())
    }

    /// `0x00482370`: clears every lock count.
    pub fn clear_locks(&mut self) {
        for e in &mut self.entries {
            e.locks = 0;
        }
    }

    /// The id → path map for the engine side ([`SoundPaths`]).
    pub fn paths(&self) -> SoundPaths {
        SoundPaths(
            (0..self.entries.len() as i32)
                .map(|id| self.path(id).and_then(Result::ok).map(Arc::from))
                .collect(),
        )
    }

    pub(super) fn entries(&self) -> &[SoundEntry] {
        &self.entries
    }
}

/// Sound id → archive path (§3), immutable: the play mode's
/// [`crate::app::sound::SoundTable`].
#[derive(Clone, Debug, Default)]
pub struct SoundPaths(Vec<Option<Arc<str>>>);

impl SoundPaths {
    pub fn get(&self, id: SoundId) -> Option<Arc<str>> {
        self.0.get(id.0 as usize).cloned().flatten()
    }
}

impl crate::app::sound::SoundTable for SoundPaths {
    fn file(&self, id: SoundId) -> Option<Arc<str>> {
        self.get(id)
    }
}
