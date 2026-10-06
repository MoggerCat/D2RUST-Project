// Spec: specs/audio/sound-table.md (§1 loading, §2 sound environment table)
// Spec: specs/data/loading.md (§3.4 runtime `.txt` sound tables)
//! The two runtime `.txt` tables of the client sound system (`loading.md`
//! §3.4): `sounds.txt` compiled with the field list of `0x00481950`
//! (`sound-table.md` §1 r2) into 142-byte records, and `soundenviron.txt`
//! into 88-byte records (§2). Compilation goes through the same compiler
//! as every data table ([`compile_table`]), so cells convert exactly as the
//! `.txt` parser does (`field-types.md` §3–§5); the typed rows are then
//! read back from the record bytes, little-endian.
//!
//! The runtime fields after the columns (group base, history, cache
//! state; `sound-table.md` §1 r3) are the client's (`d2-client::audio`).

use crate::compile::{
    compile_table, CallbackError, Callbacks, Compiled, Diagnostic, FieldCall, Linkers,
};
use crate::schema::{FieldDef, FieldType, Link};
use crate::txt::{TxtError, TxtTable};

/// `sounds.txt` record size (`sound-table.md` §1 r1).
pub const SOUND_RECORD_SIZE: usize = 0x8E;
/// `soundenviron.txt` record size (`sound-table.md` §2).
pub const SOUNDENV_RECORD_SIZE: usize = 0x58;
/// `FileName` is `str(59)` at 0.
pub const FILE_NAME_LEN: u32 = 59;

/// The `sounds.txt` field list of `0x00481950` (`sound-table.md` §1 r2),
/// in list order.
pub fn sound_fields() -> Vec<FieldDef> {
    use FieldType::{Ascii, Byte, Dword, Word};
    let f = |c: &str, t: FieldType, len: u32, off: u32| FieldDef::new(c, t, len, off, Link::None);
    vec![
        f("FileName", Ascii, FILE_NAME_LEN, 0x00),
        f("Volume", Byte, 0, 0x3C),
        f("Group Size", Byte, 0, 0x3D),
        f("Loop", Byte, 0, 0x3E),
        f("Fade In", Byte, 0, 0x3F),
        f("Fade Out", Byte, 0, 0x40),
        f("Defer Inst", Byte, 0, 0x41),
        f("Stop Inst", Byte, 0, 0x42),
        f("Duration", Word, 0, 0x43),
        f("Compound", Word, 0, 0x45),
        f("Falloff", Dword, 0, 0x47),
        f("Reverb", Byte, 0, 0x4B),
        f("Cache", Byte, 0, 0x4C),
        f("Async Only", Byte, 0, 0x4D),
        f("Priority", Byte, 0, 0x4E),
        f("Stream", Byte, 0, 0x4F),
        f("Stereo", Byte, 0, 0x50),
        f("Tracking", Byte, 0, 0x51),
        f("Solo", Byte, 0, 0x52),
        f("Music Vol", Byte, 0, 0x53),
        f("Block 1", Dword, 0, 0x54),
        f("Block 2", Dword, 0, 0x58),
        f("Block 3", Dword, 0, 0x5C),
    ]
}

/// The read `soundenviron.txt` columns with known names (`sound-table.md`
/// §2), in record order.
///
/// TODO(spec: audio/sound-table.md §2): the 12 `EAX …` columns
/// (0x24–0x50) are not named in the spec, and the spec counts 22 read
/// columns where the named ones plus 12 make 21. They matter only in
/// mixer mode 2 (EAX, not reproduced, §9), so they are not bound here.
pub fn soundenviron_fields() -> Vec<FieldDef> {
    use FieldType::{Byte, Dword};
    let f = |c: &str, t: FieldType, off: u32| FieldDef::new(c, t, 0, off, Link::None);
    vec![
        f("Song", Dword, 0x00),
        f("Day Ambience", Dword, 0x04),
        f("Night Ambience", Dword, 0x08),
        f("Day Event", Dword, 0x0C),
        f("Night Event", Dword, 0x10),
        f("Event Delay", Dword, 0x14),
        f("Indoors", Byte, 0x18),
        f("Material 1", Dword, 0x1C),
        f("Material 2", Dword, 0x20),
    ]
}

/// One `sounds.txt` record as the cells fill it (`sound-table.md` §1 r2).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SoundRow {
    /// `FileName` bytes up to the NUL (at most 59).
    pub file_name: Vec<u8>,
    pub volume: u8,
    pub group_size: u8,
    pub looped: u8,
    pub fade_in: u8,
    pub fade_out: u8,
    pub defer_inst: u8,
    pub stop_inst: u8,
    pub duration: u16,
    /// Read signed (§1 r2).
    pub compound: i16,
    pub falloff: i32,
    pub reverb: u8,
    pub cache: u8,
    pub async_only: u8,
    pub priority: u8,
    pub stream: u8,
    pub stereo: u8,
    pub tracking: u8,
    pub solo: u8,
    pub music_vol: u8,
    /// `Block 1..3`.
    pub blocks: [i32; 3],
}

impl SoundRow {
    /// Reads a compiled 142-byte record.
    pub fn from_record(r: &[u8]) -> SoundRow {
        let u16_at = |o: usize| u16::from_le_bytes([r[o], r[o + 1]]);
        let i32_at = |o: usize| i32::from_le_bytes([r[o], r[o + 1], r[o + 2], r[o + 3]]);
        let name = &r[..=FILE_NAME_LEN as usize];
        let end = name.iter().position(|&b| b == 0).unwrap_or(name.len());
        SoundRow {
            file_name: name[..end].to_vec(),
            volume: r[0x3C],
            group_size: r[0x3D],
            looped: r[0x3E],
            fade_in: r[0x3F],
            fade_out: r[0x40],
            defer_inst: r[0x41],
            stop_inst: r[0x42],
            duration: u16_at(0x43),
            compound: u16_at(0x45) as i16,
            falloff: i32_at(0x47),
            reverb: r[0x4B],
            cache: r[0x4C],
            async_only: r[0x4D],
            priority: r[0x4E],
            stream: r[0x4F],
            stereo: r[0x50],
            tracking: r[0x51],
            solo: r[0x52],
            music_vol: r[0x53],
            blocks: [i32_at(0x54), i32_at(0x58), i32_at(0x5C)],
        }
    }
}

/// One `soundenviron.txt` record (`sound-table.md` §2). The sound columns
/// hold `sounds.txt` line indices.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SoundEnvironRow {
    pub song: i32,
    pub day_ambience: i32,
    pub night_ambience: i32,
    pub day_event: i32,
    pub night_event: i32,
    pub event_delay: i32,
    pub indoors: u8,
    pub material_1: i32,
    pub material_2: i32,
}

impl SoundEnvironRow {
    /// Reads a compiled 88-byte record.
    pub fn from_record(r: &[u8]) -> SoundEnvironRow {
        let i32_at = |o: usize| i32::from_le_bytes([r[o], r[o + 1], r[o + 2], r[o + 3]]);
        SoundEnvironRow {
            song: i32_at(0x00),
            day_ambience: i32_at(0x04),
            night_ambience: i32_at(0x08),
            day_event: i32_at(0x0C),
            night_event: i32_at(0x10),
            event_delay: i32_at(0x14),
            indoors: r[0x18],
            material_1: i32_at(0x1C),
            material_2: i32_at(0x20),
        }
    }
}

/// A compiled sound table: typed rows (row `i` = data line `i`, the sound
/// id) and the compiler's diagnostics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rows<T> {
    pub rows: Vec<T>,
    pub diagnostics: Vec<Diagnostic>,
}

/// The sound field lists use no linker, key or callback type, so the
/// compiler never calls these.
struct NoCallbacks;

impl Callbacks for NoCallbacks {
    fn key(&mut self, _: &FieldDef, _: &[u8]) -> u16 {
        0
    }
    fn field(&mut self, _: FieldCall<'_>) -> Result<(), CallbackError> {
        Ok(())
    }
}

fn compile(
    file: &str,
    txt: &TxtTable,
    fields: &[FieldDef],
    size: usize,
) -> Result<Compiled, TxtError> {
    compile_table(
        file,
        txt,
        fields,
        size,
        &mut Linkers::default(),
        &mut NoCallbacks,
    )
}

/// `sounds.txt` → rows (`sound-table.md` §1 r1–r2).
pub fn compile_sounds(file: &str, txt: &TxtTable) -> Result<Rows<SoundRow>, TxtError> {
    let c = compile(file, txt, &sound_fields(), SOUND_RECORD_SIZE)?;
    Ok(Rows {
        rows: (0..c.count)
            .map(|i| SoundRow::from_record(c.record(i)))
            .collect(),
        diagnostics: c.diagnostics,
    })
}

/// `soundenviron.txt` → rows (`sound-table.md` §2).
pub fn compile_soundenviron(file: &str, txt: &TxtTable) -> Result<Rows<SoundEnvironRow>, TxtError> {
    let c = compile(file, txt, &soundenviron_fields(), SOUNDENV_RECORD_SIZE)?;
    Ok(Rows {
        rows: (0..c.count)
            .map(|i| SoundEnvironRow::from_record(c.record(i)))
            .collect(),
        diagnostics: c.diagnostics,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const HEADER: &str = "Sound\tIndex\tFileName\tVolume\tGroup Size\tLoop\tFade In\tFade Out\tDefer Inst\tStop Inst\tDuration\tCompound\tFalloff\tReverb\tCache\tAsync Only\tPriority\tStream\tStereo\tTracking\tSolo\tMusic Vol\tBlock 1\tBlock 2\tBlock 3";

    fn table(lines: &[&str]) -> TxtTable {
        let mut s = String::from(HEADER);
        s.push_str("\r\n");
        for l in lines {
            s.push_str(l);
            s.push_str("\r\n");
        }
        TxtTable::parse("sounds.txt", s.as_bytes()).unwrap()
    }

    // Covers: specs/audio/sound-table.md §1 r1, §1 r2, §1 text, §1 row1, §1 row2, §1 row3, §1 row4, §1 row5, §1 row6, §1 row7, §1 row8, §1 row9, §1 row10, §1 row11, §1 row12
    #[test]
    fn sounds_columns_and_ids() {
        let t = table(&[
            "none\t0\tnone.wav\t0\t0\t0\t0\t0\t0\t0\t0\t0\t0\t0\t0\t0\t0\t0\t0\t0\t0\t0\t-1\t-1\t-1",
            "cursor_pass\t1\tcursor\\pass.wav\t255\t3\t1\t2\t4\t1\t1\t300\t-1\t2\t1\t1\t1\t100\t1\t1\t1\t1\t1\t7\t-1\t9",
        ]);
        let rows = compile_sounds("sounds.txt", &t).unwrap();
        assert_eq!(rows.rows.len(), 2);
        let r = &rows.rows[1];
        assert_eq!(r.file_name, b"cursor\\pass.wav");
        assert_eq!(
            (r.volume, r.group_size, r.looped, r.fade_in, r.fade_out),
            (255, 3, 1, 2, 4)
        );
        assert_eq!((r.defer_inst, r.stop_inst, r.duration), (1, 1, 300));
        assert_eq!((r.compound, r.falloff, r.reverb, r.cache), (-1, 2, 1, 1));
        assert_eq!(
            (r.async_only, r.priority, r.stream, r.stereo, r.tracking),
            (1, 100, 1, 1, 1)
        );
        assert_eq!((r.solo, r.music_vol, r.blocks), (1, 1, [7, -1, 9]));
        assert_eq!(rows.rows[0].file_name, b"none.wav");
        assert_eq!(rows.rows[0].blocks, [-1, -1, -1]);
    }

    // Covers: specs/audio/sound-table.md §1 r2
    #[test]
    fn file_name_cut_at_59_and_missing_columns_zero() {
        let long = "a".repeat(70);
        let s = format!("FileName\tVolume\r\n{long}\t9\r\n");
        let t = TxtTable::parse("sounds.txt", s.as_bytes()).unwrap();
        let rows = compile_sounds("sounds.txt", &t).unwrap();
        assert_eq!(rows.rows[0].file_name.len(), 59);
        assert_eq!(rows.rows[0].volume, 9);
        assert_eq!(rows.rows[0].priority, 0);
        assert_eq!(rows.rows[0].blocks, [0, 0, 0]);
    }

    // Covers: specs/audio/sound-table.md §2
    #[test]
    fn soundenviron_columns() {
        let s = "Handle\tIndex\tSong\tDay Ambience\tNight Ambience\tDay Event\tNight Event\tEvent Delay\tIndoors\tMaterial 1\tMaterial 2\r\n\
                 x\t0\t4657\t1\t2\t3\t4\t250\t1\t5\t6\r\n";
        let t = TxtTable::parse("soundenviron.txt", s.as_bytes()).unwrap();
        let rows = compile_soundenviron("soundenviron.txt", &t).unwrap();
        assert_eq!(
            rows.rows[0],
            SoundEnvironRow {
                song: 4657,
                day_ambience: 1,
                night_ambience: 2,
                day_event: 3,
                night_event: 4,
                event_delay: 250,
                indoors: 1,
                material_1: 5,
                material_2: 6,
            }
        );
    }
}
