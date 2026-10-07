// Spec: specs/formats/native-assets.md §3.4 (finding and checking the root), §5 (runtime switch)
//! The native source: a converted root (`manifest.toml`, `files.tsv`,
//! `base/`, `mods/`) read through the layer stack (§6), returning the
//! *decoded* asset ([`NativeAsset`], the typed-loader option of §5 r1).
//! [`decode_original`] gives the same enum from the original bytes, so a
//! caller has one code path for the MPQ and the native source.
//!
//! Tables (§5 r3): [`NativeTables`] serves the `d2-data` table loader. Each
//! `.bin` it hands out is the compile of the native `.txt` set (after the
//! mods' table patches, in layer order), with `_bin-overrides.toml`
//! applied; the loader's checks then run unchanged.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use d2_data::bin::{LoadError, TableFiles};
use d2_data::compile_set::compile_all;
use d2_data::patch::{self, PatchData};
use d2_data::schema::schema;
use d2_data::strings::{StringTables, StringsError};
use d2_formats::animdata::AnimData;
use d2_formats::cof::Cof;
use d2_formats::dc6::Dc6;
use d2_formats::dcc::Dcc;
use d2_formats::ds1::Ds1;
use d2_formats::dt1::Dt1;
use d2_formats::font::FontTable;
use d2_formats::palette::{Palette, Pl2};
use d2_formats::tbl::StringTable;
use d2_formats::wav::Wav;
use sha2::{Digest, Sha256};

use crate::expfield::ExpFieldData;
use crate::kind::{FileStore, NativeKind};
use crate::layers::{Layer, LayerError, LayerStack};
use crate::manifest::{Manifest, FORMAT_VERSION};
use crate::tables::{BinOverrides, OVERRIDES_PATH};

/// What the player is told to do when the root is unusable (§3.4 r2).
pub const RERUN: &str =
    "run the converter again: cargo run -p d2-convert -- convert --install <D2 dir> --out <native dir>";

/// Every kind this build reads, with its `native_version` (§3.3 `kinds`).
pub const KNOWN_KINDS: &[(&str, u32)] = &[
    ("animdata", 1),
    ("cof", 1),
    ("dc6", 1),
    ("dcc", 1),
    ("ds1", 1),
    ("dt1", 1),
    ("excel", 1),
    ("expfield", 1),
    ("font", 1),
    ("pal", 1),
    ("pl2", 1),
    ("tbl", 1),
    ("wav", 1),
];

/// The kind of a canonical source path (§1.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum AssetKind {
    Dc6,
    Dcc,
    Cof,
    Dt1,
    Ds1,
    Pal,
    Pl2,
    Tbl,
    Font,
    Wav,
    Excel,
    AnimData,
    ExpField,
}

impl AssetKind {
    /// The kind of canonical path `p`, or `None` for a kind not converted.
    pub fn of(p: &str) -> Option<AssetKind> {
        let ext = p.rsplit_once('.').map(|(_, e)| e)?;
        Some(match ext {
            "dc6" => AssetKind::Dc6,
            "dcc" => AssetKind::Dcc,
            "cof" => AssetKind::Cof,
            "dt1" => AssetKind::Dt1,
            "ds1" => AssetKind::Ds1,
            "pl2" => AssetKind::Pl2,
            "wav" => AssetKind::Wav,
            "dat" if p.ends_with("/pal.dat") => AssetKind::Pal,
            "d2" if p == "data/global/animdata.d2" => AssetKind::AnimData,
            "d2" if p.ends_with("/expfield.d2") || p == "expfield.d2" => AssetKind::ExpField,
            "tbl" if p.starts_with("data/local/font/") => AssetKind::Font,
            "tbl" => AssetKind::Tbl,
            "txt" if p.starts_with("data/global/excel/") => AssetKind::Excel,
            _ => return None,
        })
    }

    /// The native file whose layer owns the asset (§3.2, §6 r2): every
    /// other file of the asset is read from that same layer.
    pub fn primary(self, p: &str) -> String {
        match self {
            AssetKind::Wav | AssetKind::Excel => p.to_owned(),
            AssetKind::Pal => format!("{p}.pal"),
            AssetKind::Tbl | AssetKind::AnimData => format!("{p}.tsv"),
            _ => format!("{p}.toml"),
        }
    }
}

/// A decoded asset, as both sources return it (§5 r1).
#[derive(Debug, Clone, PartialEq)]
// One value per load, moved once into its asset: boxing buys nothing.
#[allow(clippy::large_enum_variant)]
pub enum NativeAsset {
    Dc6(Dc6),
    Dcc(Dcc),
    Cof(Cof),
    Dt1(Dt1),
    Ds1(Ds1),
    Pal(Palette),
    Pl2(Pl2),
    Tbl(StringTable),
    Font(FontTable),
    Wav(Wav),
    /// The `.txt` bytes (tables compile from them, §2.8).
    Excel(Vec<u8>),
    AnimData(AnimData),
    ExpField(ExpFieldData),
}

/// Decodes the original bytes of canonical path `p` with the `d2-formats`
/// readers (the MPQ side of §5 r1). `None` for a kind not converted.
pub fn decode_original(p: &str, bytes: &[u8]) -> Option<Result<NativeAsset, String>> {
    let e = |x: d2_formats::FormatError| x.to_string();
    let kind = AssetKind::of(p)?;
    Some(match kind {
        AssetKind::Dc6 => Dc6::parse(bytes).map(NativeAsset::Dc6).map_err(e),
        AssetKind::Dcc => Dcc::parse(bytes).map(NativeAsset::Dcc).map_err(e),
        AssetKind::Cof => Cof::parse(bytes).map(NativeAsset::Cof).map_err(e),
        AssetKind::Dt1 => Dt1::parse(bytes).map(NativeAsset::Dt1).map_err(e),
        AssetKind::Ds1 => Ds1::parse(bytes).map(NativeAsset::Ds1).map_err(e),
        AssetKind::Pal => Palette::parse(bytes).map(NativeAsset::Pal).map_err(e),
        AssetKind::Pl2 => Pl2::parse(bytes).map(NativeAsset::Pl2).map_err(e),
        AssetKind::Tbl => StringTable::parse(bytes).map(NativeAsset::Tbl).map_err(e),
        AssetKind::Font => FontTable::parse(bytes).map(NativeAsset::Font).map_err(e),
        AssetKind::Wav => Wav::parse(bytes).map(NativeAsset::Wav).map_err(e),
        AssetKind::Excel => Ok(NativeAsset::Excel(bytes.to_vec())),
        AssetKind::AnimData => AnimData::parse(bytes).map(NativeAsset::AnimData).map_err(e),
        AssetKind::ExpField => ExpFieldData::from_bytes(bytes)
            .map(NativeAsset::ExpField)
            .map_err(|x| x.to_string()),
    })
    .map(|r| r.map_err(|m| format!("{p}: {m}")))
}

/// Why a native root cannot be used. Every message names the root or file
/// and, for the root, tells the player to re-run the converter (§3.4 r2).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SourceError {
    #[error("native assets in {root}: {reason}; {RERUN}")]
    Root { root: String, reason: String },
    #[error(transparent)]
    Layer(#[from] LayerError),
}

/// Lowercase hex SHA-256 (the manifest's `files_sha256` spelling).
pub fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// A converted root with its layer stack (§3.4, §5, §6).
#[derive(Debug, Clone)]
pub struct NativeSource {
    pub root: PathBuf,
    pub manifest: Manifest,
    pub layers: LayerStack,
}

/// A [`FileStore`] over one layer (a pair never mixes layers, §6 r2).
struct LayerStore<'a> {
    stack: &'a LayerStack,
    layer: &'a Layer,
}

impl FileStore for LayerStore<'_> {
    fn read(&self, path: &str) -> Option<Vec<u8>> {
        self.stack.read_in(self.layer, path).and_then(Result::ok)
    }
}

impl NativeSource {
    /// Opens `root`: `manifest.toml` known and loadable (§3.4 r2), its
    /// `files_sha256` equal to the hash of `files.tsv` (§3.4 r3), and the
    /// layer stack of `mods/order.toml` (§6).
    pub fn open(root: &Path) -> Result<NativeSource, SourceError> {
        let fail = |reason: String| SourceError::Root {
            root: root.display().to_string(),
            reason,
        };
        let text = std::fs::read_to_string(root.join("manifest.toml"))
            .map_err(|e| fail(format!("manifest.toml: {e}")))?;
        let manifest = Manifest::from_toml(&text).map_err(|e| fail(e.to_string()))?;
        let known: BTreeMap<String, u32> = KNOWN_KINDS
            .iter()
            .map(|&(k, v)| (k.to_owned(), v))
            .collect();
        manifest
            .check_loadable(&known)
            .map_err(|e| fail(e.to_string()))?;
        let tsv =
            std::fs::read(root.join("files.tsv")).map_err(|e| fail(format!("files.tsv: {e}")))?;
        if sha256_hex(&tsv) != manifest.files_sha256 {
            return Err(fail(
                "files.tsv does not match manifest.toml files_sha256".into(),
            ));
        }
        let layers = LayerStack::open(root)?;
        Ok(NativeSource {
            root: root.to_path_buf(),
            manifest,
            layers,
        })
    }

    /// The layer owning the asset at canonical path `p`, if any.
    fn owner(&self, p: &str) -> Option<(AssetKind, &Layer)> {
        let kind = AssetKind::of(p)?;
        let layer = self.layers.find(&kind.primary(p))?;
        Some((kind, layer))
    }

    /// Whether the asset at canonical path `p` has a native file.
    pub fn contains(&self, p: &str) -> bool {
        self.owner(p).is_some()
    }

    /// The decoded asset at canonical path `p`, from the last layer that
    /// has it. `None` when no layer has it (the caller's load error, §5
    /// r4); `Some(Err)` names the file when a reader refuses it.
    pub fn read_native(&self, p: &str) -> Option<Result<NativeAsset, String>> {
        let (kind, layer) = self.owner(p)?;
        let store = LayerStore {
            stack: &self.layers,
            layer,
        };
        let text = |name: String| -> Result<String, String> {
            let b = store
                .read(&name)
                .ok_or_else(|| format!("{name}: missing in layer `{}`", layer.name))?;
            String::from_utf8(b).map_err(|_| format!("{name}: not UTF-8"))
        };
        let t = |e: crate::toml_kinds::TextError| e.to_string();
        let n = |e: crate::kind::NativeError| e.to_string();
        Some((|| match kind {
            AssetKind::Dc6 => Dc6::read(p, &store).map(NativeAsset::Dc6).map_err(n),
            AssetKind::Dcc => Dcc::read(p, &store).map(NativeAsset::Dcc).map_err(n),
            AssetKind::Dt1 => Dt1::read(p, &store).map(NativeAsset::Dt1).map_err(n),
            AssetKind::Pal => Palette::read(p, &store).map(NativeAsset::Pal).map_err(n),
            AssetKind::Pl2 => Pl2::read(p, &store).map(NativeAsset::Pl2).map_err(n),
            AssetKind::ExpField => ExpFieldData::read(p, &store)
                .map(NativeAsset::ExpField)
                .map_err(n),
            AssetKind::Cof => crate::toml_kinds::read_cof(p, &text(format!("{p}.toml"))?)
                .map(NativeAsset::Cof)
                .map_err(t),
            AssetKind::Font => crate::toml_kinds::read_font(p, &text(format!("{p}.toml"))?)
                .map(NativeAsset::Font)
                .map_err(t),
            AssetKind::Ds1 => crate::toml_kinds::read_ds1(p, &text(format!("{p}.toml"))?)
                .map(NativeAsset::Ds1)
                .map_err(t),
            AssetKind::Tbl => {
                crate::tbl::read_tbl(p, &text(format!("{p}.tsv"))?, &text(format!("{p}.toml"))?)
                    .map(NativeAsset::Tbl)
                    .map_err(t)
            }
            AssetKind::AnimData => crate::animdata::read_animdata(p, &text(format!("{p}.tsv"))?)
                .map(NativeAsset::AnimData)
                .map_err(t),
            AssetKind::Wav => {
                let b = store.read(p).ok_or_else(|| format!("{p}: unreadable"))?;
                crate::wav::read_wav(p, &b).map(NativeAsset::Wav).map_err(t)
            }
            AssetKind::Excel => store
                .read(p)
                .map(NativeAsset::Excel)
                .ok_or_else(|| format!("{p}: unreadable")),
        })())
    }

    /// The table source of §5 r3 for `language` (`eng`).
    pub fn tables(&self, language: &str) -> Result<NativeTables, LoadError> {
        NativeTables::build(self, language)
    }
}

/// Canonical path of an excel file name (`Armor.bin` → `data/global/excel/armor.bin`).
fn excel_canon(file: &str) -> String {
    format!("data/global/excel/{}", file.to_ascii_lowercase())
}

/// Canonical form of an archive name (`\` → `/`, lowercase).
pub fn fold(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            '\\' => '/',
            c => c.to_ascii_lowercase(),
        })
        .collect()
}

/// The table source over a native root (§5 r3): implements
/// `d2_data::bin::TableFiles`, so `bin::load_from` runs its checks on the
/// compiled bytes exactly as on the archive's `.bin`.
#[derive(Debug, Clone)]
pub struct NativeTables {
    lod: bool,
    strings: BTreeMap<String, StringTable>,
    /// Excel `.txt` bytes (after patches), by lowercase file name.
    txt: BTreeMap<String, Vec<u8>>,
    /// Compiled `.bin` bytes (count + records), by lowercase file name.
    bins: BTreeMap<String, Vec<u8>>,
}

fn source_err(file: &str, detail: impl ToString) -> LoadError {
    LoadError::Source {
        file: file.to_owned(),
        detail: detail.to_string(),
    }
}

impl NativeTables {
    fn build(src: &NativeSource, language: &str) -> Result<NativeTables, LoadError> {
        let mut t = NativeTables {
            lod: src.manifest.lod,
            strings: BTreeMap::new(),
            txt: BTreeMap::new(),
            bins: BTreeMap::new(),
        };
        // Strings first: the compile resolves string keys (§5 r3).
        for name in ["string.tbl", "patchstring.tbl", "expansionstring.tbl"] {
            let p = format!("data/local/lng/{language}/{name}");
            match src.read_native(&p) {
                Some(Ok(NativeAsset::Tbl(table))) => {
                    t.strings.insert(p, table);
                }
                Some(Ok(_)) => unreachable!("tbl path reads as tbl"),
                Some(Err(e)) => return Err(source_err(&p, e)),
                None => {}
            }
        }
        let strings = StringTables::load_from(&t, language, t.lod)?;

        // Base `.txt` files (excel never comes from a mod, §6 r3).
        let base = &src.layers.layers[0];
        let mut read_base = |file: &str| -> Result<Option<(String, Vec<u8>)>, String> {
            let p = excel_canon(file);
            match src.layers.read_in(base, &p) {
                None => Ok(None),
                Some(Ok(b)) => Ok(Some(("native".to_owned(), b))),
                Some(Err(e)) => Err(e.to_string()),
            }
        };
        // Mod table patches, in layer order (§6 r3).
        let patches = src
            .layers
            .patches()
            .map_err(|e| source_err(crate::layers::ORDER_FILE, e))?;
        let mut patched: BTreeMap<String, Vec<u8>> = BTreeMap::new();
        if !patches.is_empty() {
            let rules = patch::rules();
            let mut data = PatchData::from_base(&rules, &mut read_base)
                .map_err(|f| source_err("table patches", findings_text(&f)))?;
            let mut layers = Vec::new();
            for (pos, (path, bytes)) in patches.iter().enumerate() {
                let (layer, findings) = patch::parse_layer(path, bytes, pos);
                if patch::has_errors(&findings) {
                    return Err(source_err(path, findings_text(&findings)));
                }
                layers.push(layer);
            }
            let findings = patch::apply_stack(&mut data, &layers, crate::layers::ORDER_FILE);
            if patch::has_errors(&findings) {
                return Err(source_err("table patches", findings_text(&findings)));
            }
            for (table, rule) in data.tables.iter().zip(&rules) {
                patched.insert(rule.txt_name.to_ascii_lowercase(), table.render());
            }
        }
        let mut read_txt = |file: &str| -> Result<Option<(String, Vec<u8>)>, String> {
            match patched.get(&file.to_ascii_lowercase()) {
                Some(b) => Ok(Some(("native".to_owned(), b.clone()))),
                None => read_base(file),
            }
        };
        let compiled =
            compile_all(&mut read_txt, &strings).map_err(|e| source_err("compile", e))?;
        let overrides = match src.layers.read_in(base, OVERRIDES_PATH) {
            None => BinOverrides::default(),
            Some(r) => {
                let b = r.map_err(|e| source_err(OVERRIDES_PATH, e))?;
                let text =
                    String::from_utf8(b).map_err(|_| source_err(OVERRIDES_PATH, "not UTF-8"))?;
                BinOverrides::from_toml(OVERRIDES_PATH, &text)
                    .map_err(|e| source_err(OVERRIDES_PATH, e))?
            }
        };
        for def in schema().called() {
            let Some(c) = compiled.table(&def.name) else {
                continue;
            };
            let mut records = c.compiled.records.clone();
            overrides
                .apply(&def.name, c.compiled.record_size, &mut records)
                .map_err(|e| source_err(OVERRIDES_PATH, e))?;
            let mut bin = (c.compiled.count as u32).to_le_bytes().to_vec();
            bin.extend_from_slice(&records);
            t.bins.insert(def.bin_name.to_ascii_lowercase(), bin);
        }
        for (buffer, bytes) in &compiled.buffers {
            t.bins.insert(
                format!("{}.bin", buffer.name()).to_ascii_lowercase(),
                bytes.clone(),
            );
        }
        // Every other base `.txt` (the runtime sound tables, §3.4 there).
        for name in ["sounds.txt", "soundenviron.txt"] {
            if let Some((_, b)) = read_txt(name).map_err(|e| source_err(name, e))? {
                t.txt.insert(name.to_owned(), b);
            }
        }
        Ok(t)
    }
}

fn findings_text(f: &[patch::Finding]) -> String {
    f.iter()
        .filter(|f| f.code.is_error())
        .map(|f| format!("{f:?}"))
        .collect::<Vec<_>>()
        .join("; ")
}

impl TableFiles for NativeTables {
    fn read_excel(&self, file: &str) -> Result<Option<(String, Vec<u8>)>, LoadError> {
        let key = file.to_ascii_lowercase();
        let hit = self.bins.get(&key).or_else(|| self.txt.get(&key));
        Ok(hit.map(|b| ("native".to_owned(), b.clone())))
    }

    fn lod(&self) -> bool {
        self.lod
    }

    fn string_table(&self, path: &str) -> Result<StringTable, StringsError> {
        self.strings
            .get(&fold(path))
            .cloned()
            .ok_or_else(|| StringsError::Source {
                file: path.to_owned(),
                detail: "no native string table".into(),
            })
    }
}

/// The native format version this build reads (re-exported for callers).
pub const NATIVE_FORMAT_VERSION: u32 = FORMAT_VERSION;
