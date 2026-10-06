// Spec: specs/audio/triggers.md §7 (object-sounds.tsv), §10 r2 (npc-speech.tsv)
// Spec: specs/audio/object-sounds.tsv
// Spec: specs/audio/npc-speech.tsv
//! The two static `Game.exe` tables of `triggers.md`, read from the spec
//! TSVs (d2rs-own data, M05) with a strict parser: an unknown header, a
//! bad cell or an out-of-range value is an error naming its line and
//! column (M07).

use std::sync::OnceLock;

/// `specs/audio/object-sounds.tsv` (§7).
pub const OBJECT_SOUNDS_TSV: &str = include_str!("../../../../../specs/audio/object-sounds.tsv");
/// `specs/audio/npc-speech.tsv` (§10 r2).
pub const NPC_SPEECH_TSV: &str = include_str!("../../../../../specs/audio/npc-speech.tsv");

/// Header of `object-sounds.tsv`.
pub const OBJECT_HEADER: [&str; 14] = [
    "class",
    "mode0",
    "mode1",
    "mode2",
    "mode3",
    "mode4",
    "mode5",
    "mode6",
    "mode7",
    "loop_a",
    "loop_a_mode",
    "loop_b",
    "loop_b_mode",
    "ordered",
];
/// Header of `npc-speech.tsv`.
pub const NPC_HEADER: [&str; 3] = ["order", "key", "sound"];

/// Largest object class of the record table (§7 r1).
pub const MAX_OBJECT_CLASS: i32 = 572;
/// Every id in both tables is below this (`sounds` line count; Test
/// vectors, real).
pub const SOUND_COUNT: i32 = 4699;

/// A table error, by 1-based line and 0-based column.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TableError {
    #[error("{table}: header {found:?} is not {expected:?}")]
    Header {
        table: &'static str,
        found: String,
        expected: String,
    },
    #[error("{table} line {line}: {cells} cells, expected {expected}")]
    Cells {
        table: &'static str,
        line: usize,
        cells: usize,
        expected: usize,
    },
    #[error("{table} line {line} column {col}: bad value {value:?}")]
    Value {
        table: &'static str,
        line: usize,
        col: usize,
        value: String,
    },
    #[error("{table} line {line}: key {key} not ascending / not as expected")]
    Order {
        table: &'static str,
        line: usize,
        key: i32,
    },
    #[error("{table}: no rows")]
    Empty { table: &'static str },
}

/// One `object-sounds.tsv` row (§7).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ObjectSoundRow {
    pub class: i32,
    /// `mode0`–`mode7` transition ids.
    pub modes: [i32; 8],
    pub loop_a: i32,
    pub loop_a_mode: u8,
    pub loop_b: i32,
    pub loop_b_mode: u8,
    /// The shared well record (`0x00729328`).
    pub ordered: bool,
}

impl ObjectSoundRow {
    /// The record without its class (for counting distinct records).
    pub fn record(&self) -> ([i32; 8], i32, u8, i32, u8, bool) {
        (
            self.modes,
            self.loop_a,
            self.loop_a_mode,
            self.loop_b,
            self.loop_b_mode,
            self.ordered,
        )
    }
}

/// The object record table (`[0x007295F8 + 4·c]`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ObjectSounds {
    rows: Vec<ObjectSoundRow>,
}

/// One `npc-speech.tsv` row (§10 r2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NpcSpeechRow {
    pub order: i32,
    /// Dialog text id.
    pub key: i32,
    pub sound: i32,
}

/// The dialog line table (`0x0072B0E0`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NpcSpeech {
    rows: Vec<NpcSpeechRow>,
}

fn rows<'a>(
    table: &'static str,
    text: &'a str,
    header: &[&str],
) -> Result<Vec<(usize, Vec<&'a str>)>, TableError> {
    let mut lines = text.lines().enumerate();
    let found = lines.next().map(|(_, l)| l).unwrap_or("");
    if found.split('\t').collect::<Vec<_>>() != header {
        return Err(TableError::Header {
            table,
            found: found.to_owned(),
            expected: header.join("\t"),
        });
    }
    let mut out = Vec::new();
    for (i, l) in lines {
        let cells: Vec<&str> = l.split('\t').collect();
        if cells.len() != header.len() {
            return Err(TableError::Cells {
                table,
                line: i + 1,
                cells: cells.len(),
                expected: header.len(),
            });
        }
        out.push((i + 1, cells));
    }
    if out.is_empty() {
        return Err(TableError::Empty { table });
    }
    Ok(out)
}

fn int(
    table: &'static str,
    line: usize,
    col: usize,
    s: &str,
    range: std::ops::RangeInclusive<i32>,
) -> Result<i32, TableError> {
    // Strict: decimal digits only (no sign, no spaces, no leading zeros).
    let bad = || TableError::Value {
        table,
        line,
        col,
        value: s.to_owned(),
    };
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) || (s.len() > 1 && s.starts_with('0'))
    {
        return Err(bad());
    }
    let v: i32 = s.parse().map_err(|_| bad())?;
    if range.contains(&v) {
        Ok(v)
    } else {
        Err(bad())
    }
}

impl ObjectSounds {
    pub fn parse(text: &str) -> Result<Self, TableError> {
        const T: &str = "object-sounds.tsv";
        let mut out: Vec<ObjectSoundRow> = Vec::new();
        for (line, c) in rows(T, text, &OBJECT_HEADER)? {
            let id = |col: usize| int(T, line, col, c[col], 0..=SOUND_COUNT - 1);
            let class = int(T, line, 0, c[0], 0..=MAX_OBJECT_CLASS)?;
            if out.last().is_some_and(|r| r.class >= class) {
                return Err(TableError::Order {
                    table: T,
                    line,
                    key: class,
                });
            }
            let mut modes = [0; 8];
            for (i, m) in modes.iter_mut().enumerate() {
                *m = id(1 + i)?;
            }
            out.push(ObjectSoundRow {
                class,
                modes,
                loop_a: id(9)?,
                loop_a_mode: int(T, line, 10, c[10], 0..=8)? as u8,
                loop_b: id(11)?,
                loop_b_mode: int(T, line, 12, c[12], 0..=8)? as u8,
                ordered: int(T, line, 13, c[13], 0..=1)? == 1,
            });
        }
        Ok(ObjectSounds { rows: out })
    }

    /// The spec table, parsed once. The parse is checked by the tests.
    pub fn spec() -> &'static ObjectSounds {
        static T: OnceLock<ObjectSounds> = OnceLock::new();
        T.get_or_init(|| ObjectSounds::parse(OBJECT_SOUNDS_TSV).expect("object-sounds.tsv"))
    }

    /// Record of class c; `None` for a class without a record.
    pub fn get(&self, class: i32) -> Option<&ObjectSoundRow> {
        self.rows
            .binary_search_by_key(&class, |r| r.class)
            .ok()
            .map(|i| &self.rows[i])
    }

    pub fn rows(&self) -> &[ObjectSoundRow] {
        &self.rows
    }
}

impl NpcSpeech {
    pub fn parse(text: &str) -> Result<Self, TableError> {
        const T: &str = "npc-speech.tsv";
        let mut out: Vec<NpcSpeechRow> = Vec::new();
        for (line, c) in rows(T, text, &NPC_HEADER)? {
            let order = int(T, line, 0, c[0], 0..=i32::MAX)?;
            if order as usize != out.len() {
                return Err(TableError::Order {
                    table: T,
                    line,
                    key: order,
                });
            }
            out.push(NpcSpeechRow {
                order,
                key: int(T, line, 1, c[1], 0..=0xFFFF)?,
                sound: int(T, line, 2, c[2], 0..=SOUND_COUNT - 1)?,
            });
        }
        Ok(NpcSpeech { rows: out })
    }

    /// The spec table, parsed once. The parse is checked by the tests.
    pub fn spec() -> &'static NpcSpeech {
        static T: OnceLock<NpcSpeech> = OnceLock::new();
        T.get_or_init(|| NpcSpeech::parse(NPC_SPEECH_TSV).expect("npc-speech.tsv"))
    }

    /// Sound of the first row whose key = `key` (first wins), else 0.
    pub fn sound(&self, key: i32) -> i32 {
        self.rows
            .iter()
            .find(|r| r.key == key)
            .map_or(0, |r| r.sound)
    }

    pub fn rows(&self) -> &[NpcSpeechRow] {
        &self.rows
    }
}
