// Spec: specs/formats/native-assets.md §2.1, §2.3 r4, §2.4
//! The text-based native kinds that are TOML sidecars: COF, font tables and
//! DS1 levels, plus the writer and reader helpers the other text kinds of
//! this crate (`tbl`, `wav`, `animdata`, `tables`) share.
//!
//! Writers emit TOML by hand so keys come out in the order the spec lists
//! them, with one spelling per value (§2.1 r6, §4.2 r3). Readers are strict
//! (M07): a wrong `native` kind or version, an unknown or missing key, a
//! value out of range or a hex value in the wrong spelling is an error
//! naming the file.

use std::fmt::{Debug, Display, Write as _};

use d2_formats::cof::{Cof, CofLayer, COMPONENTS};
use d2_formats::ds1::{Ds1, Ds1Group, Ds1Object, Ds1Path, Ds1PathPoint};
use d2_formats::font::{FontTable, Glyph};

/// `native_version` of every kind in this crate (§2.1 r7).
pub const NATIVE_VERSION: i64 = 1;

/// A native read, write or check failure, naming the file (M07).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{file}: {detail}")]
pub struct TextError {
    pub file: String,
    pub detail: String,
}

impl TextError {
    pub fn new(file: &str, detail: impl Into<String>) -> TextError {
        TextError {
            file: file.to_owned(),
            detail: detail.into(),
        }
    }
}

// ---------------------------------------------------------------- writing

/// Escapes `s` as a TOML basic string, quotes included.
pub(crate) fn toml_str(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 2);
    o.push('"');
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\t' => o.push_str("\\t"),
            '\r' => o.push_str("\\r"),
            c if (c as u32) < 0x20 || c as u32 == 0x7f => {
                let _ = write!(o, "\\u{:04x}", c as u32);
            }
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

/// `"0x…"` with exactly `digits` lowercase hex digits (§2.1 r6).
pub(crate) fn hex_str(v: u64, digits: usize) -> String {
    format!("\"0x{v:0digits$x}\"")
}

/// Byte `b` as U+00`b` (§2.1 r5).
pub(crate) fn latin1(bytes: &[u8]) -> String {
    bytes.iter().map(|&b| char::from(b)).collect()
}

/// The reverse of [`latin1`]; a character above U+00FF is an error.
pub(crate) fn unlatin1(file: &str, what: &str, s: &str) -> Result<Vec<u8>, TextError> {
    s.chars()
        .map(|c| {
            u8::try_from(u32::from(c)).map_err(|_| {
                TextError::new(
                    file,
                    format!("{what}: character U+{:04X} above U+00FF", c as u32),
                )
            })
        })
        .collect()
}

/// Line-based TOML emitter.
#[derive(Default)]
pub(crate) struct Out(pub String);

impl Out {
    pub fn header(kind: &str) -> Out {
        let mut o = Out::default();
        o.str("native", kind);
        o.int("native_version", NATIVE_VERSION);
        o
    }
    pub fn line(&mut self, s: &str) {
        self.0.push_str(s);
        self.0.push('\n');
    }
    pub fn raw(&mut self, key: &str, value: impl Display) {
        let _ = writeln!(self.0, "{key} = {value}");
    }
    pub fn int(&mut self, key: &str, v: impl Display) {
        self.raw(key, v);
    }
    pub fn boolean(&mut self, key: &str, v: bool) {
        self.raw(key, v);
    }
    pub fn str(&mut self, key: &str, v: &str) {
        self.raw(key, toml_str(v));
    }
    pub fn hex(&mut self, key: &str, v: u64, digits: usize) {
        self.raw(key, hex_str(v, digits));
    }
    pub fn ints<I: IntoIterator<Item = T>, T: Display>(&mut self, key: &str, v: I) {
        let items: Vec<String> = v.into_iter().map(|x| x.to_string()).collect();
        self.raw(key, format!("[{}]", items.join(", ")));
    }
    pub fn strs<'a, I: IntoIterator<Item = &'a str>>(&mut self, key: &str, v: I) {
        let items: Vec<String> = v.into_iter().map(toml_str).collect();
        self.raw(key, format!("[{}]", items.join(", ")));
    }
    /// A blank line, then a table header such as `[[layer]]`.
    pub fn table(&mut self, name: &str) {
        self.line("");
        self.line(name);
    }
}

// ---------------------------------------------------------------- reading

/// A TOML table being consumed: every key read is removed, and
/// [`Tab::finish`] fails on any key left over (unknown keys are errors).
pub(crate) struct Tab {
    file: String,
    ctx: String,
    t: toml::Table,
}

impl Tab {
    /// Parses a sidecar and checks its `native` kind and `native_version`.
    pub fn parse(file: &str, text: &str, kind: &str) -> Result<Tab, TextError> {
        let t: toml::Table = text
            .parse()
            .map_err(|e: toml::de::Error| TextError::new(file, format!("not valid TOML: {e}")))?;
        let mut tab = Tab {
            file: file.to_owned(),
            ctx: String::new(),
            t,
        };
        let found = tab.string("native")?;
        if found != kind {
            return Err(tab.err(format!("`native = {found:?}`, expected {kind:?}")));
        }
        let v = tab.int("native_version", i64::MIN, i64::MAX)?;
        if v != NATIVE_VERSION {
            return Err(tab.err(format!("unknown native_version {v}")));
        }
        Ok(tab)
    }

    pub fn err(&self, detail: impl Display) -> TextError {
        let at = if self.ctx.is_empty() {
            String::new()
        } else {
            format!("{}: ", self.ctx)
        };
        TextError::new(&self.file, format!("{at}{detail}"))
    }

    fn take(&mut self, key: &str) -> Result<toml::Value, TextError> {
        self.t
            .remove(key)
            .ok_or_else(|| self.err(format!("missing key `{key}`")))
    }

    pub fn has(&self, key: &str) -> bool {
        self.t.contains_key(key)
    }

    pub fn int(&mut self, key: &str, min: i64, max: i64) -> Result<i64, TextError> {
        match self.take(key)? {
            toml::Value::Integer(v) if (min..=max).contains(&v) => Ok(v),
            toml::Value::Integer(v) => Err(self.err(format!("`{key}` = {v} out of range"))),
            _ => Err(self.err(format!("`{key}` is not an integer"))),
        }
    }
    pub fn u8(&mut self, key: &str) -> Result<u8, TextError> {
        Ok(self.int(key, 0, i64::from(u8::MAX))? as u8)
    }
    pub fn u16(&mut self, key: &str) -> Result<u16, TextError> {
        Ok(self.int(key, 0, i64::from(u16::MAX))? as u16)
    }
    pub fn u32(&mut self, key: &str) -> Result<u32, TextError> {
        Ok(self.int(key, 0, i64::from(u32::MAX))? as u32)
    }
    pub fn i32(&mut self, key: &str) -> Result<i32, TextError> {
        Ok(self.int(key, i64::from(i32::MIN), i64::from(i32::MAX))? as i32)
    }
    pub fn string(&mut self, key: &str) -> Result<String, TextError> {
        match self.take(key)? {
            toml::Value::String(s) => Ok(s),
            _ => Err(self.err(format!("`{key}` is not a string"))),
        }
    }
    pub fn boolean(&mut self, key: &str) -> Result<bool, TextError> {
        match self.take(key)? {
            toml::Value::Boolean(b) => Ok(b),
            _ => Err(self.err(format!("`{key}` is not a boolean"))),
        }
    }
    /// `"0x…"` with exactly `digits` lowercase hex digits (§2.1 r6).
    pub fn hex(&mut self, key: &str, digits: usize) -> Result<u64, TextError> {
        let s = self.string(key)?;
        parse_hex(&s, digits).ok_or_else(|| {
            self.err(format!(
                "`{key}` = {s:?}: expected \"0x\" and exactly {digits} lowercase hex digits"
            ))
        })
    }
    pub fn int_array(&mut self, key: &str, min: i64, max: i64) -> Result<Vec<i64>, TextError> {
        match self.take(key)? {
            toml::Value::Array(a) => a
                .into_iter()
                .map(|v| match v {
                    toml::Value::Integer(i) if (min..=max).contains(&i) => Ok(i),
                    _ => Err(self.err(format!("`{key}`: element is not an integer in range"))),
                })
                .collect(),
            _ => Err(self.err(format!("`{key}` is not an array"))),
        }
    }
    pub fn u8_array(&mut self, key: &str) -> Result<Vec<u8>, TextError> {
        Ok(self
            .int_array(key, 0, i64::from(u8::MAX))?
            .into_iter()
            .map(|v| v as u8)
            .collect())
    }
    pub fn str_array(&mut self, key: &str) -> Result<Vec<String>, TextError> {
        match self.take(key)? {
            toml::Value::Array(a) => a
                .into_iter()
                .map(|v| match v {
                    toml::Value::String(s) => Ok(s),
                    _ => Err(self.err(format!("`{key}`: element is not a string"))),
                })
                .collect(),
            _ => Err(self.err(format!("`{key}` is not an array"))),
        }
    }
    /// An array of tables (`[[key]]`); a missing key is an empty list.
    pub fn tables(&mut self, key: &str) -> Result<Vec<Tab>, TextError> {
        let Some(v) = self.t.remove(key) else {
            return Ok(Vec::new());
        };
        let toml::Value::Array(a) = v else {
            return Err(self.err(format!("`{key}` is not an array of tables")));
        };
        a.into_iter()
            .enumerate()
            .map(|(i, v)| match v {
                toml::Value::Table(t) => Ok(Tab {
                    file: self.file.clone(),
                    ctx: format!("{}[[{key}]] #{i}", self.prefix()),
                    t,
                }),
                _ => Err(self.err(format!("`{key}` #{i} is not a table"))),
            })
            .collect()
    }
    /// A sub-table (`[key]`) if present.
    pub fn table_opt(&mut self, key: &str) -> Result<Option<Tab>, TextError> {
        match self.t.remove(key) {
            None => Ok(None),
            Some(toml::Value::Table(t)) => Ok(Some(Tab {
                file: self.file.clone(),
                ctx: format!("{}[{key}]", self.prefix()),
                t,
            })),
            Some(_) => Err(self.err(format!("`{key}` is not a table"))),
        }
    }
    fn prefix(&self) -> String {
        if self.ctx.is_empty() {
            String::new()
        } else {
            format!("{} ", self.ctx)
        }
    }
    /// Fails on any key that was not read.
    pub fn finish(self) -> Result<(), TextError> {
        match self.t.keys().next() {
            None => Ok(()),
            Some(k) => Err(self.err(format!("unknown key `{k}`"))),
        }
    }
}

/// `"0x…"` with exactly `digits` lowercase hex digits.
pub(crate) fn parse_hex(s: &str, digits: usize) -> Option<u64> {
    let body = s.strip_prefix("0x")?;
    if body.len() != digits
        || !body
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return None;
    }
    u64::from_str_radix(body, 16).ok()
}

/// Parses `digits` lowercase hex digits (no prefix) per cell.
pub(crate) fn parse_bare_hex(s: &str, digits: usize) -> Option<u64> {
    if s.len() != digits
        || !s
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return None;
    }
    u64::from_str_radix(s, 16).ok()
}

/// The first line where the pretty `Debug` forms of `a` and `b` differ.
pub(crate) fn first_difference<T: Debug>(a: &T, b: &T) -> String {
    let (sa, sb) = (format!("{a:#?}"), format!("{b:#?}"));
    let (mut la, mut lb) = (sa.lines(), sb.lines());
    let mut n = 1;
    loop {
        match (la.next(), lb.next()) {
            (Some(x), Some(y)) if x == y => n += 1,
            (x, y) => {
                return format!(
                    "first difference at debug line {n}: original `{}`, native `{}`",
                    x.unwrap_or("<end>").trim(),
                    y.unwrap_or("<end>").trim()
                )
            }
        }
    }
}

fn invalid_struct(file: &str, detail: impl Into<String>) -> TextError {
    TextError::new(file, format!("not representable: {}", detail.into()))
}

// -------------------------------------------------------------------- COF

/// Writes a COF sidecar (§2.4). The struct must hold the invariants the
/// parser guarantees (counts agree with the vectors).
pub fn write_cof(file: &str, c: &Cof) -> Result<String, TextError> {
    let (l, f, d) = (
        usize::from(c.layers_count),
        usize::from(c.frames),
        usize::from(c.directions),
    );
    if c.layers.len() != l {
        return Err(invalid_struct(file, "layers_count differs from layers"));
    }
    if c.events.len() != f {
        return Err(invalid_struct(file, "events differ from frames"));
    }
    if c.draw_order.len() != d * f * l {
        return Err(invalid_struct(
            file,
            "draw_order is not directions × frames × layers",
        ));
    }
    let mut o = Out::header("cof");
    o.int("version", c.version);
    o.hex("unknown", u64::from(u32::from_be_bytes(c.unknown)), 8);
    o.int("frames", c.frames);
    o.int("directions", c.directions);
    o.int("x_min", c.x_min);
    o.int("x_max", c.x_max);
    o.int("y_min", c.y_min);
    o.int("y_max", c.y_max);
    o.int("animation_rate", c.animation_rate);
    o.ints("events", c.events.iter());
    o.ints("event_padding", c.event_padding.iter());
    // One row per direction: frames × layers component IDs, back to front.
    let row = f * l;
    let rows: Vec<String> = (0..d)
        .map(|i| {
            let r = &c.draw_order[i * row..(i + 1) * row];
            let items: Vec<String> = r.iter().map(|b| b.to_string()).collect();
            format!("[{}]", items.join(", "))
        })
        .collect();
    o.raw("draw_order", format!("[{}]", rows.join(", ")));
    for ly in &c.layers {
        o.table("[[layer]]");
        o.int("component", ly.component);
        o.int("shadow", ly.shadow);
        o.int("selectable", ly.selectable);
        o.int("override_translucency", ly.override_translucency);
        o.int("new_translucency", ly.new_translucency);
        o.str("weapon_class", &latin1(&ly.weapon_class));
    }
    Ok(o.0)
}

/// Reads a COF sidecar back into the decoded struct.
pub fn read_cof(file: &str, text: &str) -> Result<Cof, TextError> {
    let mut t = Tab::parse(file, text, "cof")?;
    let version = t.u8("version")?;
    let unknown = (t.hex("unknown", 8)? as u32).to_be_bytes();
    let frames = t.u8("frames")?;
    let directions = t.u8("directions")?;
    let x_min = t.i32("x_min")?;
    let x_max = t.i32("x_max")?;
    let y_min = t.i32("y_min")?;
    let y_max = t.i32("y_max")?;
    let animation_rate = t.u32("animation_rate")?;
    let events = t.u8_array("events")?;
    let event_padding = t.u8_array("event_padding")?;
    let order_rows = match t.take("draw_order")? {
        toml::Value::Array(a) => a,
        _ => return Err(t.err("`draw_order` is not an array")),
    };
    let mut layers = Vec::new();
    for mut ly in t.tables("layer")? {
        let component = ly.u8("component")?;
        if usize::from(component) >= COMPONENTS {
            return Err(ly.err(format!("component {component}")));
        }
        let shadow = ly.u8("shadow")?;
        let selectable = ly.u8("selectable")?;
        let override_translucency = ly.u8("override_translucency")?;
        let new_translucency = ly.u8("new_translucency")?;
        let wc = ly.string("weapon_class")?;
        let wc = unlatin1(file, "weapon_class", &wc)?;
        let weapon_class: [u8; 4] = wc
            .try_into()
            .map_err(|_| ly.err("weapon_class is not 4 characters"))?;
        ly.finish()?;
        layers.push(CofLayer {
            component,
            shadow,
            selectable,
            override_translucency,
            new_translucency,
            weapon_class,
        });
    }
    let layers_count = u8::try_from(layers.len()).map_err(|_| t.err("more than 255 layers"))?;
    if events.len() != usize::from(frames) {
        return Err(t.err(format!("{} events for {frames} frames", events.len())));
    }
    if order_rows.len() != usize::from(directions) {
        return Err(t.err(format!(
            "draw_order has {} rows for {directions} directions",
            order_rows.len()
        )));
    }
    let row_len = usize::from(frames) * layers.len();
    let mut draw_order = Vec::with_capacity(row_len * order_rows.len());
    for (i, r) in order_rows.into_iter().enumerate() {
        let toml::Value::Array(items) = r else {
            return Err(t.err(format!("draw_order row {i} is not an array")));
        };
        if items.len() != row_len {
            return Err(t.err(format!(
                "draw_order row {i} has {} entries, expected {row_len}",
                items.len()
            )));
        }
        for v in items {
            match v {
                toml::Value::Integer(b) if (0..COMPONENTS as i64).contains(&b) => {
                    draw_order.push(b as u8)
                }
                _ => return Err(t.err(format!("draw_order row {i}: not a component ID"))),
            }
        }
    }
    t.finish()?;
    Ok(Cof {
        layers_count,
        frames,
        directions,
        version,
        unknown,
        x_min,
        x_max,
        y_min,
        y_max,
        animation_rate,
        layers,
        events,
        event_padding,
        draw_order,
    })
}

/// C-STRUCT for COF: write, read back, compare (§4.3).
pub fn check_cof(file: &str, original: &Cof) -> Result<(), TextError> {
    let back = read_cof(file, &write_cof(file, original)?)?;
    if &back == original {
        Ok(())
    } else {
        Err(TextError::new(file, first_difference(original, &back)))
    }
}

// ------------------------------------------------------------------- font

/// Writes a font-table sidecar (§2.4).
pub fn write_font(_file: &str, f: &FontTable) -> Result<String, TextError> {
    let mut o = Out::header("font");
    o.int("version", f.version);
    o.int("unknown", f.unknown);
    o.int("count", f.count);
    o.int("height", f.height);
    o.int("width", f.width);
    for g in &f.glyphs {
        o.table("[[glyph]]");
        o.int("code", g.code);
        o.int("unknown1", g.unknown1);
        o.int("width", g.width);
        o.int("height", g.height);
        o.int("unknown2", g.unknown2);
        o.int("unknown3", g.unknown3);
        o.int("frame", g.frame);
        o.int("unknown5", g.unknown5);
    }
    Ok(o.0)
}

pub fn read_font(file: &str, text: &str) -> Result<FontTable, TextError> {
    let mut t = Tab::parse(file, text, "font")?;
    let version = t.u16("version")?;
    let unknown = t.u16("unknown")?;
    let count = t.u16("count")?;
    let height = t.u8("height")?;
    let width = t.u8("width")?;
    let mut glyphs = Vec::new();
    for mut g in t.tables("glyph")? {
        glyphs.push(Glyph {
            code: g.u16("code")?,
            unknown1: g.u8("unknown1")?,
            width: g.u8("width")?,
            height: g.u8("height")?,
            unknown2: g.u8("unknown2")?,
            unknown3: g.u16("unknown3")?,
            frame: g.u16("frame")?,
            unknown5: g.u32("unknown5")?,
        });
        g.finish()?;
    }
    t.finish()?;
    Ok(FontTable {
        version,
        unknown,
        count,
        height,
        width,
        glyphs,
    })
}

pub fn check_font(file: &str, original: &FontTable) -> Result<(), TextError> {
    let back = read_font(file, &write_font(file, original)?)?;
    if &back == original {
        Ok(())
    } else {
        Err(TextError::new(file, first_difference(original, &back)))
    }
}

// -------------------------------------------------------------------- DS1

/// Most rows a grid may have when the width is 0 (a header can claim 2^32
/// empty rows; the sidecar would be gigabytes of empty strings).
const MAX_EMPTY_ROWS: u32 = 1 << 20;

fn grid_rows(g: &[u32], width: u32, height: u32) -> String {
    let w = width as usize;
    let rows: Vec<String> = (0..height as usize)
        .map(|y| {
            let cells: Vec<String> = g[y * w..(y + 1) * w]
                .iter()
                .map(|c| format!("{c:08x}"))
                .collect();
            toml_str(&cells.join(" "))
        })
        .collect();
    format!("[{}]", rows.join(", "))
}

/// Writes a DS1 sidecar (§2.3 r4). `width` / `height` are the struct's
/// values (the header plus one), and a layer has `height` rows of `width`
/// cells.
pub fn write_ds1(file: &str, d: &Ds1) -> Result<String, TextError> {
    let cells = u64::from(d.width) * u64::from(d.height);
    let layers: Vec<&Vec<u32>> = d
        .walls
        .iter()
        .chain(&d.orientations)
        .chain(&d.floors)
        .chain(std::iter::once(&d.shadow))
        .chain(d.tags.iter())
        .collect();
    if layers.iter().any(|l| l.len() as u64 != cells) {
        return Err(invalid_struct(file, "a layer is not width × height cells"));
    }
    if d.orientations.len() != d.walls.len() {
        return Err(invalid_struct(
            file,
            "orientation layers differ from wall layers",
        ));
    }
    if d.width == 0 && d.height > MAX_EMPTY_ROWS {
        return Err(invalid_struct(file, "empty grid with too many rows"));
    }
    let mut o = Out::header("ds1");
    o.int("version", d.version);
    o.int("width", d.width);
    o.int("height", d.height);
    o.int("act", d.act);
    o.int("tag_type", d.tag_type);
    if let Some(u) = d.unknown_header {
        o.hex("unknown_header", u64::from_be_bytes(u), 16);
    }
    let names: Vec<String> = d.files.iter().map(|f| latin1(f)).collect();
    o.strs("files", names.iter().map(String::as_str));
    if let Some(u) = d.unknown_groups {
        o.hex("unknown_groups", u64::from(u32::from_be_bytes(u)), 8);
    }
    o.boolean("groups_truncated", d.groups_truncated);
    o.str("trailing", &hex_bytes(&d.trailing));

    // File order: each wall followed by its orientation, floors, shadow, tag.
    let emit = |o: &mut Out, kind: &str, index: usize, g: &[u32]| {
        o.table("[[layer]]");
        o.str("kind", kind);
        o.int("index", index);
        o.raw("rows", grid_rows(g, d.width, d.height));
    };
    for i in 0..d.walls.len() {
        emit(&mut o, "wall", i, &d.walls[i]);
        emit(&mut o, "orientation", i, &d.orientations[i]);
    }
    for (i, g) in d.floors.iter().enumerate() {
        emit(&mut o, "floor", i, g);
    }
    emit(&mut o, "shadow", 0, &d.shadow);
    if let Some(g) = &d.tags {
        emit(&mut o, "tag", 0, g);
    }
    for ob in &d.objects {
        o.table("[[object]]");
        o.int("kind", ob.kind);
        o.int("id", ob.id);
        o.int("x", ob.x);
        o.int("y", ob.y);
        o.hex("flags", u64::from(ob.flags), 8);
    }
    for g in &d.groups {
        o.table("[[group]]");
        o.int("x", g.x);
        o.int("y", g.y);
        o.int("width", g.width);
        o.int("height", g.height);
        o.int("unknown", g.unknown);
    }
    for p in &d.paths {
        o.table("[[path]]");
        o.int("x", p.x);
        o.int("y", p.y);
        for pt in &p.points {
            o.table("[[path.point]]");
            o.int("x", pt.x);
            o.int("y", pt.y);
            o.int("action", pt.action);
        }
    }
    Ok(o.0)
}

/// Lowercase hex of `b`, no prefix (the `trailing` string).
pub(crate) fn hex_bytes(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

pub(crate) fn unhex_bytes(s: &str) -> Option<Vec<u8>> {
    if !s.len().is_multiple_of(2) {
        return None;
    }
    (0..s.len())
        .step_by(2)
        .map(|i| parse_bare_hex(s.get(i..i + 2)?, 2).map(|v| v as u8))
        .collect()
}

fn read_grid(t: &mut Tab, width: u32, height: u32) -> Result<Vec<u32>, TextError> {
    let rows = t.str_array("rows")?;
    if rows.len() != height as usize {
        return Err(t.err(format!("{} rows, expected {height}", rows.len())));
    }
    let mut cells = Vec::with_capacity(width as usize * height as usize);
    for (y, row) in rows.iter().enumerate() {
        let before = cells.len();
        if width > 0 {
            for c in row.split(' ') {
                let v = parse_bare_hex(c, 8).ok_or_else(|| {
                    t.err(format!("row {y}: cell {c:?} is not 8 lowercase hex digits"))
                })?;
                cells.push(v as u32);
            }
        } else if !row.is_empty() {
            return Err(t.err(format!("row {y} has cells in a width-0 grid")));
        }
        if cells.len() - before != width as usize {
            return Err(t.err(format!(
                "row {y} has {} cells, expected {width}",
                cells.len() - before
            )));
        }
    }
    Ok(cells)
}

/// Reads a DS1 sidecar back into the decoded struct.
pub fn read_ds1(file: &str, text: &str) -> Result<Ds1, TextError> {
    let mut t = Tab::parse(file, text, "ds1")?;
    let version = t.u32("version")?;
    let width = t.u32("width")?;
    let height = t.u32("height")?;
    let act = t.u32("act")?;
    let tag_type = t.u32("tag_type")?;
    let unknown_header = if t.has("unknown_header") {
        Some(t.hex("unknown_header", 16)?.to_be_bytes())
    } else {
        None
    };
    let mut files = Vec::new();
    for name in t.str_array("files")? {
        files.push(unlatin1(file, "files", &name)?);
    }
    let unknown_groups = if t.has("unknown_groups") {
        Some((t.hex("unknown_groups", 8)? as u32).to_be_bytes())
    } else {
        None
    };
    let groups_truncated = t.boolean("groups_truncated")?;
    let trailing = t.string("trailing")?;
    let trailing =
        unhex_bytes(&trailing).ok_or_else(|| t.err("`trailing` is not lowercase hex bytes"))?;

    if width == 0 && height > MAX_EMPTY_ROWS {
        return Err(t.err("empty grid with too many rows"));
    }
    let mut walls: Vec<Vec<u32>> = Vec::new();
    let mut orientations: Vec<Vec<u32>> = Vec::new();
    let mut floors: Vec<Vec<u32>> = Vec::new();
    let mut shadow = None;
    let mut tags = None;
    for mut l in t.tables("layer")? {
        let kind = l.string("kind")?;
        let index = l.int("index", 0, i64::MAX)? as usize;
        let grid = read_grid(&mut l, width, height)?;
        let slot: &mut Vec<Vec<u32>> = match kind.as_str() {
            "wall" => &mut walls,
            "orientation" => &mut orientations,
            "floor" => &mut floors,
            "shadow" | "tag" => {
                let dst = if kind == "shadow" {
                    &mut shadow
                } else {
                    &mut tags
                };
                if index != 0 || dst.is_some() {
                    return Err(l.err(format!("{kind} layer repeated or indexed")));
                }
                *dst = Some(grid);
                l.finish()?;
                continue;
            }
            other => return Err(l.err(format!("unknown layer kind {other:?}"))),
        };
        if index != slot.len() {
            return Err(l.err(format!(
                "{kind} layer index {index}, expected {}",
                slot.len()
            )));
        }
        slot.push(grid);
        l.finish()?;
    }
    if walls.len() != orientations.len() {
        return Err(t.err("wall and orientation layer counts differ"));
    }
    let shadow = shadow.ok_or_else(|| t.err("no shadow layer"))?;

    let mut objects = Vec::new();
    for mut o in t.tables("object")? {
        objects.push(Ds1Object {
            kind: o.u32("kind")?,
            id: o.u32("id")?,
            x: o.u32("x")?,
            y: o.u32("y")?,
            flags: o.hex("flags", 8)? as u32,
        });
        o.finish()?;
    }
    let mut groups = Vec::new();
    for mut g in t.tables("group")? {
        groups.push(Ds1Group {
            x: g.u32("x")?,
            y: g.u32("y")?,
            width: g.u32("width")?,
            height: g.u32("height")?,
            unknown: g.u32("unknown")?,
        });
        g.finish()?;
    }
    let mut paths = Vec::new();
    for mut p in t.tables("path")? {
        let x = p.u32("x")?;
        let y = p.u32("y")?;
        let mut points = Vec::new();
        for mut pt in p.tables("point")? {
            points.push(Ds1PathPoint {
                x: pt.u32("x")?,
                y: pt.u32("y")?,
                action: pt.u32("action")?,
            });
            pt.finish()?;
        }
        p.finish()?;
        paths.push(Ds1Path { x, y, points });
    }
    t.finish()?;
    Ok(Ds1 {
        version,
        width,
        height,
        act,
        tag_type,
        files,
        unknown_header,
        walls,
        orientations,
        floors,
        shadow,
        tags,
        objects,
        unknown_groups,
        groups,
        groups_truncated,
        paths,
        trailing,
    })
}

pub fn check_ds1(file: &str, original: &Ds1) -> Result<(), TextError> {
    let back = read_ds1(file, &write_ds1(file, original)?)?;
    if &back == original {
        Ok(())
    } else {
        Err(TextError::new(file, first_difference(original, &back)))
    }
}

// -------------------------------------------------------------------- TSV

/// Escapes one TSV cell (§2.1 r8): `\`, tab, LF and CR only.
pub(crate) fn tsv_escape(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => o.push_str("\\\\"),
            '\t' => o.push_str("\\t"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            c => o.push(c),
        }
    }
    o
}

/// The reverse of [`tsv_escape`]; any other escape is an error.
pub(crate) fn tsv_unescape(file: &str, line: usize, s: &str) -> Result<String, TextError> {
    let mut o = String::with_capacity(s.len());
    let mut it = s.chars();
    while let Some(c) = it.next() {
        if c != '\\' {
            o.push(c);
            continue;
        }
        match it.next() {
            Some('\\') => o.push('\\'),
            Some('t') => o.push('\t'),
            Some('n') => o.push('\n'),
            Some('r') => o.push('\r'),
            other => {
                return Err(TextError::new(
                    file,
                    format!("line {line}: bad escape `\\{}`", other.unwrap_or(' ')),
                ))
            }
        }
    }
    Ok(o)
}

/// Splits a TSV file into its header and rows of cells (not unescaped).
/// The file must end with `\n` and hold no CR line ends (§2.1 r8).
pub(crate) fn tsv_rows<'a>(
    file: &str,
    text: &'a str,
    header: &str,
    columns: usize,
) -> Result<Vec<Vec<&'a str>>, TextError> {
    let body = text
        .strip_suffix('\n')
        .ok_or_else(|| TextError::new(file, "no final line end"))?;
    let mut lines = body.split('\n');
    if lines.next() != Some(header) {
        return Err(TextError::new(
            file,
            format!("header is not {:?}", header.replace('\t', "\\t")),
        ));
    }
    lines
        .enumerate()
        .map(|(i, l)| {
            let cells: Vec<&str> = l.split('\t').collect();
            if cells.len() == columns {
                Ok(cells)
            } else {
                Err(TextError::new(
                    file,
                    format!("line {}: {} cells, expected {columns}", i + 2, cells.len()),
                ))
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cof() -> Cof {
        let layer = |component, wc: &[u8; 4]| CofLayer {
            component,
            shadow: 1,
            selectable: 1,
            override_translucency: 0,
            new_translucency: 0,
            weapon_class: *wc,
        };
        Cof {
            layers_count: 2,
            frames: 3,
            directions: 2,
            version: 20,
            unknown: [1, 2, 3, 4],
            x_min: -10,
            x_max: 10,
            y_min: -20,
            y_max: 0,
            animation_rate: 256,
            layers: vec![layer(0, b"hth\0"), layer(15, b"\xe9\xffa ")],
            events: vec![0, 1, 3],
            event_padding: vec![9, 9],
            draw_order: (0..12).map(|i| (i % 16) as u8).collect(),
        }
    }

    // Covers: specs/formats/native-assets.md §2.4, §7.1 r1
    #[test]
    fn cof_round_trip() {
        let c = cof();
        let t = write_cof("a.cof.toml", &c).unwrap();
        assert!(t.starts_with(
            "native = \"cof\"\nnative_version = 1\nversion = 20\nunknown = \"0x01020304\"\n"
        ));
        assert!(t.contains("draw_order = [[0, 1, 2, 3, 4, 5], [6, 7, 8, 9, 10, 11]]"));
        assert_eq!(read_cof("a.cof.toml", &t).unwrap(), c);
        check_cof("a.cof.toml", &c).unwrap();
        assert_eq!(t, write_cof("a.cof.toml", &c).unwrap());
        // Edge: no layers, no frames, no directions.
        let e = Cof {
            layers_count: 0,
            frames: 0,
            directions: 0,
            layers: vec![],
            events: vec![],
            draw_order: vec![],
            ..cof()
        };
        check_cof("e.cof.toml", &e).unwrap();
        let e2 = Cof {
            frames: 0,
            events: vec![],
            draw_order: vec![],
            ..cof()
        };
        check_cof("e2.cof.toml", &e2).unwrap();
    }

    // Covers: specs/formats/native-assets.md §7.1 r3, §7.1 r5, §2.1 r5, §2.1 r6
    #[test]
    fn cof_perturbation_and_strict_reader() {
        let c = cof();
        let t = write_cof("a.cof.toml", &c).unwrap();
        let back = read_cof("a.cof.toml", &t.replace("y_max = 0", "y_max = 1")).unwrap();
        assert_eq!(back.y_max, 1);
        assert!(first_difference(&c, &back).contains("1,"));
        for bad in [
            t.replace("native_version = 1", "native_version = 2"),
            t.replace("\"cof\"", "\"font\""),
            t.replace("0x01020304", "0x1020304"),
            t.replace("0x01020304", "0x0102030A"),
            t.replace("version = 20", "version = 256"),
            t.replace("9, 10, 11]", "9, 10, 16]"),
            t.replace("[6, 7, 8,", "[6, 7,"),
            t.replace("events = [0, 1, 3]", "events = [0, 1]"),
            t.replace("\"hth\\u0000\"", "\"hth\""),
            t.replace("\"éÿa \"", "\"Ā\u{100}a \""),
            format!("{t}zzz = 1\n"),
            t.replace("component = 15", "component = 16"),
        ] {
            assert_ne!(bad, t, "the perturbation changed nothing");
            let e = read_cof("a.cof.toml", &bad).unwrap_err();
            assert!(e.to_string().starts_with("a.cof.toml: "), "{e}\n{bad}");
        }
        let mut bad = cof();
        bad.events.pop();
        assert!(write_cof("a", &bad).is_err());
    }

    // Covers: specs/formats/native-assets.md §2.4, §7.1 r1
    #[test]
    fn font_round_trip_and_strict() {
        let f = FontTable {
            version: 1,
            unknown: 7,
            count: 2,
            height: 16,
            width: 12,
            glyphs: vec![
                Glyph {
                    code: 0x41,
                    unknown1: 1,
                    width: 7,
                    height: 16,
                    unknown2: 2,
                    unknown3: 3,
                    frame: 33,
                    unknown5: 0xdead_beef,
                },
                Glyph {
                    code: 0xac10,
                    unknown1: 0,
                    width: 9,
                    height: 16,
                    unknown2: 0,
                    unknown3: 0,
                    frame: 200,
                    unknown5: 0,
                },
            ],
        };
        let t = write_font("f.toml", &f).unwrap();
        assert_eq!(read_font("f.toml", &t).unwrap(), f);
        check_font("f.toml", &f).unwrap();
        let none = FontTable {
            glyphs: vec![],
            count: 0,
            ..f.clone()
        };
        check_font("f.toml", &none).unwrap();
        assert!(read_font("f.toml", &t.replace("frame = 33", "frame = 70000")).is_err());
        assert!(read_font("f.toml", &t.replace("code = 65\n", "")).is_err());
        let back = read_font("f.toml", &t.replace("frame = 200", "frame = 201")).unwrap();
        assert!(first_difference(&f, &back).contains("201"));
    }

    fn ds1() -> Ds1 {
        let grid =
            |seed: u32| -> Vec<u32> { (0..6).map(|i| seed * 0x0100_0000 + i * 0x101).collect() };
        Ds1 {
            version: 18,
            width: 3,
            height: 2,
            act: 1,
            tag_type: 1,
            files: vec![b"data\\tiles\\floor.tg1".to_vec(), vec![b'a', 0xe9, 0xff]],
            unknown_header: None,
            walls: vec![grid(1), grid(2)],
            orientations: vec![grid(3), grid(4)],
            floors: vec![grid(5)],
            shadow: grid(6),
            tags: Some(grid(7)),
            objects: vec![Ds1Object {
                kind: 1,
                id: 2,
                x: 3,
                y: 4,
                flags: 0x0000_00ff,
            }],
            unknown_groups: Some([1, 0, 0, 255]),
            groups: vec![Ds1Group {
                x: 1,
                y: 2,
                width: 3,
                height: 4,
                unknown: 5,
            }],
            groups_truncated: true,
            paths: vec![Ds1Path {
                x: 9,
                y: 8,
                points: vec![
                    Ds1PathPoint {
                        x: 1,
                        y: 2,
                        action: 3,
                    },
                    Ds1PathPoint {
                        x: 4,
                        y: 5,
                        action: 1,
                    },
                ],
            }],
            trailing: vec![0, 0xab, 0xff],
        }
    }

    // Covers: specs/formats/native-assets.md §2.3 r4, §7.1 r1
    #[test]
    fn ds1_round_trip() {
        let d = ds1();
        let t = write_ds1("a.ds1.toml", &d).unwrap();
        assert!(
            t.contains("rows = [\"01000000 01000101 01000202\", \"01000303 01000404 01000505\"]")
        );
        assert!(t.contains("trailing = \"00abff\""));
        assert!(t.contains("flags = \"0x000000ff\""));
        assert_eq!(read_ds1("a.ds1.toml", &t).unwrap(), d);
        check_ds1("a.ds1.toml", &d).unwrap();
        assert_eq!(t, write_ds1("a.ds1.toml", &d).unwrap());
        // Old version: one wall, no tags, header unknown, nothing optional.
        let old = Ds1 {
            version: 9,
            tag_type: 0,
            unknown_header: Some([1, 2, 3, 4, 5, 6, 7, 8]),
            walls: vec![d.walls[0].clone()],
            orientations: vec![d.orientations[0].clone()],
            tags: None,
            unknown_groups: None,
            groups: vec![],
            groups_truncated: false,
            paths: vec![],
            objects: vec![],
            files: vec![],
            trailing: vec![],
            ..d.clone()
        };
        let t = write_ds1("o.ds1.toml", &old).unwrap();
        assert!(t.contains("unknown_header = \"0x0102030405060708\""));
        check_ds1("o.ds1.toml", &old).unwrap();
        // Empty grid.
        let empty = Ds1 {
            width: 0,
            height: 0,
            walls: vec![vec![]],
            orientations: vec![vec![]],
            floors: vec![vec![]],
            shadow: vec![],
            tags: Some(vec![]),
            ..d.clone()
        };
        check_ds1("z.ds1.toml", &empty).unwrap();
    }

    // Covers: specs/formats/native-assets.md §7.1 r3, §7.1 r5, §2.1 r5, §2.1 r6
    #[test]
    fn ds1_perturbation_and_strict_reader() {
        let d = ds1();
        let t = write_ds1("a.ds1.toml", &d).unwrap();
        let back = read_ds1("a.ds1.toml", &t.replacen("01000303", "01000304", 1)).unwrap();
        assert_ne!(back, d);
        assert!(first_difference(&d, &back).contains("difference"));
        for bad in [
            t.replace("native_version = 1", "native_version = 9"),
            t.replacen("01000303", "0100030", 1),
            t.replacen("01000303", "0100030A", 1),
            t.replacen("01000303", "010003030", 1),
            t.replace("0x000000ff", "0xff"),
            t.replace("0x000000ff", "0x000000FF"),
            t.replace("00abff", "00abf"),
            t.replace("\"data\\\\tiles", "\"Ā\\\\tiles"),
            t.replace("kind = \"shadow\"", "kind = \"roof\""),
            t.replace("index = 1\nrows", "index = 2\nrows"),
            t.replacen("rows = [\"01000000 01000101 01000202\", ", "rows = [", 1),
            format!("{t}\n[[path]]\nx = 1\ny = 2\nbogus = 3\n"),
        ] {
            assert_ne!(bad, t, "the perturbation changed nothing");
            let e = read_ds1("a.ds1.toml", &bad).unwrap_err();
            assert!(e.to_string().starts_with("a.ds1.toml: "), "{e}");
        }
        let mut bad = d.clone();
        bad.shadow.pop();
        assert!(write_ds1("a", &bad).is_err());
        let mut bad = d;
        bad.orientations.pop();
        assert!(write_ds1("a", &bad).is_err());
    }
}
