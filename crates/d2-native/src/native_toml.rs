// Spec: specs/formats/native-assets.md
//! Sidecar helpers shared by the image kinds: one spelling per value
//! (§2.1 r6), writers that emit keys in a fixed order (§4.2 r3), and a
//! strict reader that rejects unknown keys (M07).

use crate::kind::NativeError;

pub fn hex2(v: u8) -> String {
    format!("\"0x{v:02x}\"")
}
pub fn hex4(v: u16) -> String {
    format!("\"0x{v:04x}\"")
}
pub fn hex8(v: u32) -> String {
    format!("\"0x{v:08x}\"")
}

/// `[a, b, c]` of already formatted values.
pub fn array<I: IntoIterator<Item = String>>(items: I) -> String {
    format!("[{}]", items.into_iter().collect::<Vec<_>>().join(", "))
}

/// Parses `0x` + exactly `digits` lowercase hex digits.
pub fn parse_hex(s: &str, digits: usize) -> Option<u64> {
    let h = s.strip_prefix("0x")?;
    if h.len() != digits
        || !h
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return None;
    }
    u64::from_str_radix(h, 16).ok()
}

/// A TOML table read key by key; [`Fields::finish`] rejects leftovers.
pub struct Fields {
    file: String,
    ctx: String,
    table: toml::Table,
}

impl Fields {
    pub fn parse(file: &str, text: &str) -> Result<Fields, NativeError> {
        let table: toml::Table = text
            .parse()
            .map_err(|e: toml::de::Error| NativeError::new(file, format!("bad TOML: {e}")))?;
        Ok(Fields {
            file: file.to_string(),
            ctx: String::new(),
            table,
        })
    }

    pub fn file(&self) -> &str {
        &self.file
    }

    pub fn err(&self, msg: impl std::fmt::Display) -> NativeError {
        let at = if self.ctx.is_empty() {
            String::new()
        } else {
            format!("{}: ", self.ctx)
        };
        NativeError::new(&self.file, format!("{at}{msg}"))
    }

    fn take(&mut self, key: &str) -> Result<toml::Value, NativeError> {
        self.table
            .remove(key)
            .ok_or_else(|| self.err(format!("missing key `{key}`")))
    }

    pub fn has(&self, key: &str) -> bool {
        self.table.contains_key(key)
    }

    pub fn str(&mut self, key: &str) -> Result<String, NativeError> {
        match self.take(key)? {
            toml::Value::String(s) => Ok(s),
            _ => Err(self.err(format!("`{key}` must be a string"))),
        }
    }

    pub fn bool(&mut self, key: &str) -> Result<bool, NativeError> {
        match self.take(key)? {
            toml::Value::Boolean(b) => Ok(b),
            _ => Err(self.err(format!("`{key}` must be a boolean"))),
        }
    }

    pub fn int(&mut self, key: &str) -> Result<i64, NativeError> {
        match self.take(key)? {
            toml::Value::Integer(i) => Ok(i),
            _ => Err(self.err(format!("`{key}` must be an integer"))),
        }
    }

    fn ranged(&mut self, key: &str, lo: i64, hi: i64) -> Result<i64, NativeError> {
        let v = self.int(key)?;
        if v < lo || v > hi {
            return Err(self.err(format!("`{key}` = {v} out of range {lo}..={hi}")));
        }
        Ok(v)
    }

    pub fn u32(&mut self, key: &str) -> Result<u32, NativeError> {
        self.ranged(key, 0, i64::from(u32::MAX)).map(|v| v as u32)
    }
    pub fn u16(&mut self, key: &str) -> Result<u16, NativeError> {
        self.ranged(key, 0, i64::from(u16::MAX)).map(|v| v as u16)
    }
    pub fn u8(&mut self, key: &str) -> Result<u8, NativeError> {
        self.ranged(key, 0, i64::from(u8::MAX)).map(|v| v as u8)
    }
    pub fn i32(&mut self, key: &str) -> Result<i32, NativeError> {
        self.ranged(key, i64::from(i32::MIN), i64::from(i32::MAX))
            .map(|v| v as i32)
    }
    pub fn i16(&mut self, key: &str) -> Result<i16, NativeError> {
        self.ranged(key, i64::from(i16::MIN), i64::from(i16::MAX))
            .map(|v| v as i16)
    }

    fn hex(&mut self, key: &str, digits: usize) -> Result<u64, NativeError> {
        let s = self.str(key)?;
        parse_hex(&s, digits).ok_or_else(|| {
            self.err(format!(
                "`{key}` = {s:?}: expected \"0x\" and {digits} lowercase hex digits"
            ))
        })
    }
    pub fn hex_u32(&mut self, key: &str) -> Result<u32, NativeError> {
        self.hex(key, 8).map(|v| v as u32)
    }
    pub fn hex_u16(&mut self, key: &str) -> Result<u16, NativeError> {
        self.hex(key, 4).map(|v| v as u16)
    }
    pub fn hex_u8(&mut self, key: &str) -> Result<u8, NativeError> {
        self.hex(key, 2).map(|v| v as u8)
    }

    /// An array of integers.
    pub fn ints(&mut self, key: &str) -> Result<Vec<i64>, NativeError> {
        match self.take(key)? {
            toml::Value::Array(a) => a
                .into_iter()
                .map(|v| match v {
                    toml::Value::Integer(i) => Ok(i),
                    _ => Err(self.err(format!("`{key}` must hold integers"))),
                })
                .collect(),
            _ => Err(self.err(format!("`{key}` must be an array"))),
        }
    }

    /// An array of strings.
    pub fn strs(&mut self, key: &str) -> Result<Vec<String>, NativeError> {
        match self.take(key)? {
            toml::Value::Array(a) => a
                .into_iter()
                .map(|v| match v {
                    toml::Value::String(s) => Ok(s),
                    _ => Err(self.err(format!("`{key}` must hold strings"))),
                })
                .collect(),
            _ => Err(self.err(format!("`{key}` must be an array"))),
        }
    }

    /// A sub-table.
    pub fn table(&mut self, key: &str) -> Result<Fields, NativeError> {
        match self.take(key)? {
            toml::Value::Table(t) => Ok(self.child(key, t)),
            _ => Err(self.err(format!("`{key}` must be a table"))),
        }
    }

    /// An optional sub-table (e.g. `[encoding]`, §2.1 r4).
    pub fn opt_table(&mut self, key: &str) -> Result<Option<Fields>, NativeError> {
        if self.has(key) {
            self.table(key).map(Some)
        } else {
            Ok(None)
        }
    }

    /// An array of tables, in order.
    pub fn tables(&mut self, key: &str) -> Result<Vec<Fields>, NativeError> {
        if !self.has(key) {
            return Ok(Vec::new());
        }
        match self.take(key)? {
            toml::Value::Array(a) => a
                .into_iter()
                .enumerate()
                .map(|(i, v)| match v {
                    toml::Value::Table(t) => Ok(self.child(&format!("{key}[{i}]"), t)),
                    _ => Err(self.err(format!("`{key}` must hold tables"))),
                })
                .collect(),
            _ => Err(self.err(format!("`{key}` must be an array of tables"))),
        }
    }

    fn child(&self, name: &str, table: toml::Table) -> Fields {
        let ctx = if self.ctx.is_empty() {
            name.to_string()
        } else {
            format!("{}.{name}", self.ctx)
        };
        Fields {
            file: self.file.clone(),
            ctx,
            table,
        }
    }

    /// The `native` / `native_version` header (§2.1 r7).
    pub fn header(&mut self, kind: &str, version: u32) -> Result<(), NativeError> {
        let k = self.str("native")?;
        if k != kind {
            return Err(self.err(format!("native = {k:?}, expected {kind:?}")));
        }
        let v = self.int("native_version")?;
        if v != i64::from(version) {
            return Err(self.err(format!(
                "native_version {v} unknown (this build: {version})"
            )));
        }
        Ok(())
    }

    /// Errors on any key not read (unknown input is an error, M07).
    pub fn finish(&self) -> Result<(), NativeError> {
        if let Some(k) = self.table.keys().next() {
            return Err(self.err(format!("unknown key `{k}`")));
        }
        Ok(())
    }
}
