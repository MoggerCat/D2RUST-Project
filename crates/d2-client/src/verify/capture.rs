// Spec: specs/render/capture.md (§3–§7), traces/FORMAT.md (Render captures)
//! Reader of the 1.14d frame captures written by
//! `tools/trace-recorder/record_frames.py`: the raw JSON-lines file
//! (format `frames-raw-1`, capture.md §5) and its 8-bit palettized PNGs.
//! Strict (M07): a missing or other `format`, an unknown record kind, a
//! missing field, a wrong type, a footer whose counts disagree with the
//! lines, or a PNG that is not 8-bit color type 3 is an error naming the
//! line or file, never a default.
//!
//! Also the two checks that need no renderer: the hashes of §6
//! ([`sha256_hex`], [`Image::check`]) and the stability rule of §7
//! ([`stability`]).

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::io::Cursor;
use std::path::{Path, PathBuf};

use d2_formats::palette::Palette;
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

/// The raw format this build reads (capture.md §5; M20).
pub const FORMAT: &str = "frames-raw-1";

/// The video type a capture must have: GDI (capture.md §1).
pub const GDI: u32 = 1;

/// SHA-256 as lowercase hex, as the recorder writes it (`hexdigest`).
pub fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// The 768 palette bytes R, G, B for indices 0–255 (capture.md §6): what
/// `palette_sha256` hashes and what the PNG's PLTE holds.
pub fn palette_bytes(p: &Palette) -> Vec<u8> {
    p.colors.iter().flat_map(|c| [c.r, c.g, c.b]).collect()
}

/// A capture file error: where (`line 12: player.mode`, a file name) and
/// what is wrong.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{at}: {what}")]
pub struct CaptureError {
    pub at: String,
    pub what: String,
}

fn err(at: impl Into<String>, what: impl Into<String>) -> CaptureError {
    CaptureError {
        at: at.into(),
        what: what.into(),
    }
}

/// One `frames-raw-1` file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Raw {
    pub header: Header,
    /// `game` records: the game pointer the tick hook follows (hex text).
    pub games: Vec<String>,
    /// `tick` records: the server frame numbers `f`, in file order.
    pub ticks: Vec<i64>,
    /// `frame` records, in file order (refused ones included).
    pub frames: Vec<Frame>,
    pub footer: Footer,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Header {
    pub tool: String,
    pub date: String,
    pub game_exe_sha256: String,
    pub args: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Footer {
    pub ticks: i64,
    pub notes: Vec<String>,
}

/// One `frame` record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    /// 1-based line in the raw file.
    pub line: usize,
    /// `f`: the last server tick before this frame (capture.md §4); `None`
    /// before the first tick of the recording.
    pub tick: Option<i64>,
    pub video_type: u32,
    pub w: i32,
    pub h: i32,
    pub body: FrameBody,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FrameBody {
    /// The configuration was not GDI at 800 × 600 (capture.md §1).
    Refused(String),
    Captured(Box<Captured>),
}

/// A captured frame's hashes, image name and recorded state (capture.md §3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Captured {
    pub index_sha256: String,
    pub palette_sha256: String,
    /// In-game draw counter `[0x007A0494]`.
    pub draw: u32,
    /// PNG file name in the capture's image directory (absent with
    /// `--no-save`).
    pub image: Option<String>,
    pub state: State,
}

/// The state a frame was drawn from (capture.md §3, `camera.md`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct State {
    pub player: Option<Player>,
    /// View `+0x04..+0x10`; absent when the view pointer was null.
    pub view_rect: Option<[i32; 4]>,
    /// View `+0x24/+0x28`; absent when the view pointer was null.
    pub tile_origin: Option<[i32; 2]>,
    pub unit_origin: [i32; 2],
    pub open_mode: u32,
    pub shift_x: i32,
    /// Amplitude (unsigned), dx, dy.
    pub shake: [i64; 3],
    pub clear_counter: i32,
    pub res_mode: u32,
}

/// The client player unit (capture.md §3).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Player {
    pub unit_type: u32,
    pub mode: u32,
    /// `+0x44` (capture.md Open question 1).
    pub cur: i32,
    /// Path `+0/+4`, present for a player unit with a path.
    pub fixed: Option<[u32; 2]>,
    /// Path `+8/+0xC`, present with `fixed`.
    pub client: Option<[i32; 2]>,
}

impl Frame {
    pub fn captured(&self) -> Option<&Captured> {
        match &self.body {
            FrameBody::Captured(c) => Some(c),
            FrameBody::Refused(_) => None,
        }
    }
}

/// Reads a `frames-raw-1` file.
pub fn read_raw(path: &Path) -> Result<Raw, CaptureError> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| err(path.display().to_string(), e.to_string()))?;
    parse_raw(&text).map_err(|e| err(format!("{}: {}", path.display(), e.at), e.what))
}

/// Parses the text of a `frames-raw-1` file.
pub fn parse_raw(text: &str) -> Result<Raw, CaptureError> {
    let mut header = None;
    let mut footer = None;
    let (mut games, mut ticks, mut frames) = (Vec::new(), Vec::new(), Vec::new());
    for (i, line) in text.lines().enumerate() {
        let n = i + 1;
        let at = format!("line {n}");
        if footer.is_some() {
            return Err(err(at, "a record after the footer"));
        }
        let value: Value =
            serde_json::from_str(line).map_err(|e| err(&at, format!("JSON: {e}")))?;
        let rec = value
            .as_object()
            .ok_or_else(|| err(&at, "a record is a JSON object"))?;
        let r = Rec { at: &at, rec };
        let kind = r.str("k")?;
        if header.is_none() && kind != "header" {
            return Err(err(
                &at,
                format!("first record is {kind:?}, not the header"),
            ));
        }
        match kind {
            "header" => {
                if header.is_some() {
                    return Err(err(at, "a second header"));
                }
                // Version first (M20): an unknown format is reported before
                // anything it may have changed.
                let format = r.str("format")?;
                if format != FORMAT {
                    return Err(err(
                        &at,
                        format!("format {format:?} is not supported (this build reads {FORMAT:?})"),
                    ));
                }
                let args = r
                    .get("args")?
                    .as_array()
                    .ok_or_else(|| r.wrong("args", "an array of strings"))?
                    .iter()
                    .map(|a| {
                        a.as_str()
                            .map(str::to_owned)
                            .ok_or_else(|| r.wrong("args", "an array of strings"))
                    })
                    .collect::<Result<_, _>>()?;
                header = Some(Header {
                    tool: r.str("tool")?.to_owned(),
                    date: r.str("date")?.to_owned(),
                    game_exe_sha256: r.str("game_exe_sha256")?.to_owned(),
                    args,
                });
            }
            "game" => games.push(r.str("g")?.to_owned()),
            "tick" => ticks.push(r.int("f", i64::MIN, i64::MAX)?),
            "frame" => frames.push(frame(&r, n)?),
            "footer" => {
                let counts = r
                    .get("counts")?
                    .as_object()
                    .ok_or_else(|| r.wrong("counts", "an object"))?;
                // The recorder counts every emitted record by kind; the
                // lines must agree (a cut or edited file is caught here).
                let seen = [
                    ("game", games.len()),
                    ("tick", ticks.len()),
                    ("frame", frames.len()),
                ];
                for (k, v) in counts {
                    let want = v
                        .as_u64()
                        .ok_or_else(|| r.wrong(&format!("counts.{k}"), "an integer"))?;
                    let got = seen
                        .iter()
                        .find(|(name, _)| name == k)
                        .map(|&(_, c)| c)
                        .ok_or_else(|| err(&at, format!("counts.{k}: unknown record kind")))?;
                    if want != got as u64 {
                        return Err(err(
                            &at,
                            format!("counts.{k} = {want}, the file has {got} such lines"),
                        ));
                    }
                }
                for (name, c) in seen {
                    if c > 0 && !counts.contains_key(name) {
                        return Err(err(&at, format!("counts has no {name} ({c} lines)")));
                    }
                }
                let notes = r
                    .get("notes")?
                    .as_array()
                    .ok_or_else(|| r.wrong("notes", "an array of strings"))?
                    .iter()
                    .map(|a| {
                        a.as_str()
                            .map(str::to_owned)
                            .ok_or_else(|| r.wrong("notes", "an array of strings"))
                    })
                    .collect::<Result<_, _>>()?;
                let n_ticks = r.int("ticks", 0, i64::MAX)?;
                if n_ticks != ticks.len() as i64 {
                    return Err(err(
                        &at,
                        format!("ticks = {n_ticks}, the file has {} tick lines", ticks.len()),
                    ));
                }
                footer = Some(Footer {
                    ticks: n_ticks,
                    notes,
                });
            }
            other => return Err(err(&at, format!("unknown record kind {other:?}"))),
        }
    }
    let header = header.ok_or_else(|| err("line 1", "empty file (no header)"))?;
    let footer = footer.ok_or_else(|| err("end of file", "no footer (the recording was cut)"))?;
    Ok(Raw {
        header,
        games,
        ticks,
        frames,
        footer,
    })
}

fn frame(r: &Rec<'_>, line: usize) -> Result<Frame, CaptureError> {
    let tick = match r.get("f")? {
        Value::Null => None,
        _ => Some(r.int("f", i64::MIN, i64::MAX)?),
    };
    let video_type = r.int("video_type", 0, u32::MAX.into())? as u32;
    let w = r.int("w", i32::MIN.into(), i32::MAX.into())? as i32;
    let h = r.int("h", i32::MIN.into(), i32::MAX.into())? as i32;
    let body = if r.rec.contains_key("refused") {
        FrameBody::Refused(r.str("refused")?.to_owned())
    } else {
        if video_type != GDI || w <= 0 || h <= 0 {
            return Err(err(
                r.at,
                format!("a captured frame with video type {video_type} at {w}x{h} (capture.md §1)"),
            ));
        }
        let image = match r.rec.get("image") {
            None => None,
            Some(_) => Some(r.str("image")?.to_owned()),
        };
        FrameBody::Captured(Box::new(Captured {
            index_sha256: r.hash("index_sha256")?,
            palette_sha256: r.hash("palette_sha256")?,
            draw: r.int("draw", 0, u32::MAX.into())? as u32,
            image,
            state: state(r)?,
        }))
    };
    Ok(Frame {
        line,
        tick,
        video_type,
        w,
        h,
        body,
    })
}

fn state(r: &Rec<'_>) -> Result<State, CaptureError> {
    let i32s = |key: &str| r.ints::<2>(key, i32::MIN.into(), i32::MAX.into());
    let player = match r.rec.get("player") {
        None => None,
        Some(v) => {
            let p = Rec {
                at: &format!("{}: player", r.at),
                rec: v
                    .as_object()
                    .ok_or_else(|| r.wrong("player", "an object"))?,
            };
            let fixed = match p.rec.get("fixed") {
                None => None,
                Some(_) => Some(p.ints::<2>("fixed", 0, u32::MAX.into())?.map(|v| v as u32)),
            };
            let client = match p.rec.get("client") {
                None => None,
                Some(_) => Some(
                    p.ints::<2>("client", i32::MIN.into(), i32::MAX.into())?
                        .map(|v| v as i32),
                ),
            };
            if fixed.is_some() != client.is_some() {
                return Err(p.wrong("fixed", "fixed and client together"));
            }
            Some(Player {
                unit_type: p.int("type", 0, u32::MAX.into())? as u32,
                mode: p.int("mode", 0, u32::MAX.into())? as u32,
                cur: p.int("cur", i32::MIN.into(), i32::MAX.into())? as i32,
                fixed,
                client,
            })
        }
    };
    let view_rect = match r.rec.get("view_rect") {
        None => None,
        Some(_) => Some(
            r.ints::<4>("view_rect", i32::MIN.into(), i32::MAX.into())?
                .map(|v| v as i32),
        ),
    };
    let tile_origin = match r.rec.get("tile_origin") {
        None => None,
        Some(_) => Some(i32s("tile_origin")?.map(|v| v as i32)),
    };
    let shake = r.ints::<3>("shake", i32::MIN.into(), u32::MAX.into())?;
    // Amplitude `[0x007B9534]` is read unsigned, dx and dy signed.
    if shake[0] < 0 || shake[1..].iter().any(|&v| v > i32::MAX.into()) {
        return Err(r.wrong("shake", "[u32, i32, i32]"));
    }
    Ok(State {
        player,
        view_rect,
        tile_origin,
        unit_origin: i32s("unit_origin")?.map(|v| v as i32),
        open_mode: r.int("open_mode", 0, u32::MAX.into())? as u32,
        shift_x: r.int("shift_x", i32::MIN.into(), i32::MAX.into())? as i32,
        shake,
        clear_counter: r.int("clear_counter", i32::MIN.into(), i32::MAX.into())? as i32,
        res_mode: r.int("res_mode", 0, u32::MAX.into())? as u32,
    })
}

/// Typed field access on one record, errors naming the line and key.
struct Rec<'a> {
    at: &'a str,
    rec: &'a Map<String, Value>,
}

impl Rec<'_> {
    fn wrong(&self, key: &str, want: &str) -> CaptureError {
        err(self.at, format!("{key}: expected {want}"))
    }

    fn get(&self, key: &str) -> Result<&Value, CaptureError> {
        self.rec
            .get(key)
            .ok_or_else(|| err(self.at, format!("{key}: missing")))
    }

    fn str(&self, key: &str) -> Result<&str, CaptureError> {
        self.get(key)?
            .as_str()
            .ok_or_else(|| self.wrong(key, "a string"))
    }

    fn hash(&self, key: &str) -> Result<String, CaptureError> {
        let s = self.str(key)?;
        if s.len() != 64
            || !s
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(self.wrong(key, "64 lowercase hex digits"));
        }
        Ok(s.to_owned())
    }

    fn int_of(&self, key: &str, v: &Value, min: i64, max: i64) -> Result<i64, CaptureError> {
        let v = v.as_i64().ok_or_else(|| self.wrong(key, "an integer"))?;
        if !(min..=max).contains(&v) {
            return Err(err(
                self.at,
                format!("{key}: {v} is out of range {min}..={max}"),
            ));
        }
        Ok(v)
    }

    fn int(&self, key: &str, min: i64, max: i64) -> Result<i64, CaptureError> {
        self.int_of(key, self.get(key)?, min, max)
    }

    fn ints<const N: usize>(
        &self,
        key: &str,
        min: i64,
        max: i64,
    ) -> Result<[i64; N], CaptureError> {
        let arr = self
            .get(key)?
            .as_array()
            .filter(|a| a.len() == N)
            .ok_or_else(|| self.wrong(key, &format!("an array of {N} integers")))?;
        let mut out = [0; N];
        for (o, v) in out.iter_mut().zip(arr) {
            *o = self.int_of(key, v, min, max)?;
        }
        Ok(out)
    }
}

/// A capture PNG read back: the index bytes and the 768 palette bytes,
/// unchanged (capture.md §5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    /// `width × height` bytes, row-major, top row first.
    pub indices: Vec<u8>,
    /// 768 bytes R, G, B.
    pub palette: Vec<u8>,
}

impl Image {
    /// The integrity check of a capture: the PNG's size, index hash and
    /// palette hash equal the frame record's (capture.md §6). A failure
    /// means the PNG and the raw file disagree (an edited or mixed-up file).
    pub fn check(&self, frame: &Frame, c: &Captured) -> Result<(), String> {
        if (self.width as i64, self.height as i64) != (frame.w.into(), frame.h.into()) {
            return Err(format!(
                "PNG is {}x{}, the record {}x{}",
                self.width, self.height, frame.w, frame.h
            ));
        }
        let got = sha256_hex(&self.indices);
        if got != c.index_sha256 {
            return Err(format!(
                "PNG index hash {got} differs from the record's index_sha256 {}",
                c.index_sha256
            ));
        }
        let got = sha256_hex(&self.palette);
        if got != c.palette_sha256 {
            return Err(format!(
                "PNG palette hash {got} differs from the record's palette_sha256 {}",
                c.palette_sha256
            ));
        }
        Ok(())
    }
}

/// Decodes a capture PNG: 8-bit, color type 3 (palettized), 256 PLTE
/// entries; the indices are returned as stored (no expansion).
pub fn decode_png(bytes: &[u8]) -> Result<Image, String> {
    let mut decoder = png::Decoder::new(Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::IDENTITY);
    let mut reader = decoder.read_info().map_err(|e| format!("PNG: {e}"))?;
    let info = reader.info();
    if info.color_type != png::ColorType::Indexed || info.bit_depth != png::BitDepth::Eight {
        return Err(format!(
            "PNG is {:?} {:?}, a capture is 8-bit palettized (color type 3)",
            info.bit_depth, info.color_type
        ));
    }
    if info.interlaced {
        return Err("PNG is interlaced; a capture is not".into());
    }
    let palette = info.palette.as_ref().ok_or("PNG has no PLTE")?.to_vec();
    if palette.len() != 768 {
        return Err(format!(
            "PLTE has {} bytes, a capture has 768",
            palette.len()
        ));
    }
    let (width, height) = (info.width, info.height);
    let size = reader.output_buffer_size().ok_or("PNG frame too large")?;
    let mut buf = vec![0; size];
    let out = reader
        .next_frame(&mut buf)
        .map_err(|e| format!("PNG: {e}"))?;
    let len = width as usize * height as usize;
    if out.line_size != width as usize || out.buffer_size() != len {
        return Err(format!(
            "PNG rows are {} bytes for width {width}",
            out.line_size
        ));
    }
    buf.truncate(len);
    Ok(Image {
        width,
        height,
        indices: buf,
        palette,
    })
}

/// Reads the PNG of a captured frame from the capture's image directory.
pub fn read_image(dir: &Path, c: &Captured) -> Result<(PathBuf, Image), String> {
    let name = c
        .image
        .as_deref()
        .ok_or("the record has no image (recorded with --no-save)")?;
    let path = dir.join(name);
    let bytes = std::fs::read(&path).map_err(|e| format!("reading {}: {e}", path.display()))?;
    let image = decode_png(&bytes).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok((path, image))
}

/// The state key of capture.md §7: frames with equal keys were drawn from
/// equal recorded state. Same fields as the recorder's key.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct StateKey {
    pub player: Option<Player>,
    pub tile_origin: Option<[i32; 2]>,
    pub unit_origin: [i32; 2],
    pub shake: [i64; 3],
    pub open_mode: u32,
    pub palette_sha256: String,
}

impl StateKey {
    pub fn of(c: &Captured) -> Self {
        let s = &c.state;
        StateKey {
            player: s.player.clone(),
            tile_origin: s.tile_origin,
            unit_origin: s.unit_origin,
            shake: s.shake,
            open_mode: s.open_mode,
            palette_sha256: c.palette_sha256.clone(),
        }
    }
}

/// One state group: the draws with that key and the index hashes seen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Group {
    pub key: StateKey,
    pub draws: Vec<u32>,
    pub hashes: BTreeSet<String>,
}

/// The result of the stability rule (capture.md §7).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stability {
    /// All groups, in key order.
    pub groups: Vec<Group>,
}

impl Stability {
    /// Groups seen at least twice.
    pub fn repeated(&self) -> usize {
        self.groups.iter().filter(|g| g.draws.len() >= 2).count()
    }

    /// Groups whose frames differ.
    pub fn differing(&self) -> impl Iterator<Item = &Group> {
        self.groups.iter().filter(|g| g.hashes.len() > 1)
    }

    /// §7 holds: no group has differing frames and at least two keys were
    /// seen at least twice each.
    pub fn holds(&self) -> bool {
        self.differing().next().is_none() && self.repeated() >= 2
    }
}

impl fmt::Display for Stability {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} state groups, {} seen at least twice, {} with differing frames",
            self.groups.len(),
            self.repeated(),
            self.differing().count()
        )
    }
}

/// Groups captured frames by [`StateKey`] (capture.md §7). `hash` gives
/// each frame's index hash (the record's, or a re-hash of its PNG).
pub fn stability<'a>(frames: impl IntoIterator<Item = (&'a Captured, String)>) -> Stability {
    let mut groups: BTreeMap<StateKey, Group> = BTreeMap::new();
    for (c, hash) in frames {
        let key = StateKey::of(c);
        let g = groups.entry(key.clone()).or_insert_with(|| Group {
            key,
            draws: Vec::new(),
            hashes: BTreeSet::new(),
        });
        g.draws.push(c.draw);
        g.hashes.insert(hash);
    }
    Stability {
        groups: groups.into_values().collect(),
    }
}

/// Ticks shared by more than one captured frame (capture.md §4: a pause or
/// a skipped tick), as `(tick, draws)`.
pub fn repeated_ticks(frames: &[Frame]) -> Vec<(i64, Vec<u32>)> {
    let mut by_tick: BTreeMap<i64, Vec<u32>> = BTreeMap::new();
    for f in frames {
        if let (Some(t), Some(c)) = (f.tick, f.captured()) {
            by_tick.entry(t).or_default().push(c.draw);
        }
    }
    by_tick.into_iter().filter(|(_, d)| d.len() > 1).collect()
}
