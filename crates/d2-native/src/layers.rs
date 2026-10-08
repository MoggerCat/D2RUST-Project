// Spec: specs/formats/native-assets.md §6 (mod layering), §3.1 (layout)
//! The layer stack: `base/` first, then each enabled mod's `files/` tree in
//! the order `mods/order.toml` lists (later wins, §6 r1). A native path
//! resolves to the last layer that has it (§6 r2). Checks made when the
//! stack is opened (§6 r2, r3, r5): a mod's `mod.toml` is well formed and
//! of this build's `native_format_version`; its `requires` come earlier in
//! the order; it ships no excel `.txt` and no string-table files (rule 9:
//! patches, not copies); every sheet / tileset / PL2 sidecar has its pixel
//! half in the same layer and the other way round.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use crate::manifest::FORMAT_VERSION;

/// Name of the base layer in messages.
pub const BASE: &str = "base";
/// `mods/order.toml` (§6 r1).
pub const ORDER_FILE: &str = "mods/order.toml";
/// Where a mod keeps its table patches, under its `files/` tree (§6 r3).
pub const PATCH_DIR: &str = "patches";

/// A layer-stack error naming the mod and file (M07).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LayerError {
    #[error("{0}: {1}")]
    Io(String, String),
    #[error("{file}: {detail}")]
    Bad { file: String, detail: String },
    #[error("mod `{layer}` ships {file}: excel tables and string tables are changed by patches, not copies (rule 9)")]
    Copy { layer: String, file: String },
    #[error("mod `{layer}`: {file} has no matching {missing} in the same layer")]
    Unpaired {
        layer: String,
        file: String,
        missing: String,
    },
}

fn bad(file: impl Into<String>, detail: impl Into<String>) -> LayerError {
    LayerError::Bad {
        file: file.into(),
        detail: detail.into(),
    }
}

/// A mod's `mod.toml` (§6 r1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModInfo {
    pub name: String,
    pub version: String,
    pub native_format_version: u32,
    pub requires: Vec<String>,
}

impl ModInfo {
    /// Parses `mod.toml`. Strict: every key present and of its type.
    pub fn from_toml(file: &str, text: &str) -> Result<ModInfo, LayerError> {
        let t: toml::Table = text.parse().map_err(|e| bad(file, format!("{e}")))?;
        let s = |k: &str| {
            t.get(k)
                .and_then(|v| v.as_str())
                .map(str::to_owned)
                .ok_or_else(|| bad(file, format!("`{k}` missing or not a string")))
        };
        let v = t
            .get("native_format_version")
            .and_then(|v| v.as_integer())
            .and_then(|v| u32::try_from(v).ok())
            .ok_or_else(|| bad(file, "`native_format_version` missing or not an integer"))?;
        let requires = match t.get("requires") {
            None => Vec::new(),
            Some(r) => r
                .as_array()
                .and_then(|a| {
                    a.iter()
                        .map(|x| x.as_str().map(str::to_owned))
                        .collect::<Option<Vec<_>>>()
                })
                .ok_or_else(|| bad(file, "`requires` is not a list of strings"))?,
        };
        for k in t.keys() {
            if !["name", "version", "native_format_version", "requires"].contains(&k.as_str()) {
                return Err(bad(file, format!("unknown key `{k}`")));
            }
        }
        Ok(ModInfo {
            name: s("name")?,
            version: s("version")?,
            native_format_version: v,
            requires,
        })
    }
}

/// Parses `mods/order.toml`: `order = ["modA", "modB"]`.
pub fn parse_order(text: &str) -> Result<Vec<String>, LayerError> {
    let t: toml::Table = text.parse().map_err(|e| bad(ORDER_FILE, format!("{e}")))?;
    let order = t
        .get("order")
        .and_then(|v| v.as_array())
        .and_then(|a| {
            a.iter()
                .map(|x| x.as_str().map(str::to_owned))
                .collect::<Option<Vec<_>>>()
        })
        .ok_or_else(|| bad(ORDER_FILE, "`order` missing or not a list of strings"))?;
    let mut seen = BTreeSet::new();
    for m in &order {
        if !seen.insert(m) {
            return Err(bad(ORDER_FILE, format!("mod `{m}` listed twice")));
        }
    }
    Ok(order)
}

/// One layer: its name and the folder native paths are relative to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layer {
    pub name: String,
    pub dir: PathBuf,
    /// `None` for `base`.
    pub info: Option<ModInfo>,
}

/// The ordered layers, `base` first (§6 r1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayerStack {
    pub layers: Vec<Layer>,
}

fn io(path: &Path, e: std::io::Error) -> LayerError {
    LayerError::Io(path.display().to_string(), e.to_string())
}

/// Every file under `dir`, as `/`-joined paths relative to it, sorted.
fn walk(dir: &Path) -> Result<Vec<String>, LayerError> {
    let mut out = Vec::new();
    let mut todo = vec![(dir.to_path_buf(), String::new())];
    while let Some((d, prefix)) = todo.pop() {
        for e in std::fs::read_dir(&d).map_err(|e| io(&d, e))? {
            let e = e.map_err(|e| io(&d, e))?;
            let name = e.file_name().to_string_lossy().into_owned();
            let rel = format!("{prefix}{name}");
            if e.file_type().map_err(|err| io(&e.path(), err))?.is_dir() {
                todo.push((e.path(), format!("{rel}/")));
            } else {
                out.push(rel);
            }
        }
    }
    out.sort();
    Ok(out)
}

/// The §6 r2 / r3 checks of one mod's file list.
pub fn check_mod_files(layer: &str, files: &[String]) -> Result<(), LayerError> {
    let set: BTreeSet<&str> = files.iter().map(String::as_str).collect();
    let has_dir = |d: &str| {
        let p = format!("{d}/");
        set.range(p.as_str()..)
            .next()
            .is_some_and(|f| f.starts_with(&p))
    };
    for f in files {
        let copy = (f.starts_with("data/global/excel/") && f.ends_with(".txt"))
            || f.starts_with("data/global/excel/_bin-overrides")
            || (f.starts_with("data/local/lng/")
                && (f.ends_with(".tbl.tsv") || f.ends_with(".tbl.toml")));
        if copy {
            return Err(LayerError::Copy {
                layer: layer.into(),
                file: f.clone(),
            });
        }
        let unpaired = |missing: String| LayerError::Unpaired {
            layer: layer.into(),
            file: f.clone(),
            missing,
        };
        // Sheets, PL2 and expfield: `P.toml` + `P.png` (§2.2, §2.5, §2.9).
        for ext in [".dc6", ".dcc", ".pl2", "expfield.d2"] {
            for (have, want) in [(".toml", ".png"), (".png", ".toml")] {
                if let Some(p) = f.strip_suffix(&format!("{ext}{have}")) {
                    let other = format!("{p}{ext}{want}");
                    if !set.contains(other.as_str()) {
                        return Err(unpaired(other));
                    }
                }
            }
        }
        // Tilesets: `P.toml` + `P.d/` (§2.3).
        if let Some(p) = f.strip_suffix(".dt1.toml") {
            let d = format!("{p}.dt1.d");
            if !has_dir(&d) {
                return Err(unpaired(format!("{d}/")));
            }
        }
        if let Some((p, _)) = f.split_once(".dt1.d/") {
            let t = format!("{p}.dt1.toml");
            if !set.contains(t.as_str()) {
                return Err(unpaired(t));
            }
        }
    }
    Ok(())
}

impl LayerStack {
    /// The stack of a native root: `base/`, then the mods of
    /// `mods/order.toml` (none when the file is absent).
    pub fn open(root: &Path) -> Result<LayerStack, LayerError> {
        let order_path = root.join(ORDER_FILE);
        let order = match std::fs::read_to_string(&order_path) {
            Ok(text) => parse_order(&text)?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            Err(e) => return Err(io(&order_path, e)),
        };
        LayerStack::with_mods(root, &order)
    }

    /// The stack of `root` with the mods `order` (folders under `mods/`).
    pub fn with_mods(root: &Path, order: &[String]) -> Result<LayerStack, LayerError> {
        let mut layers = vec![Layer {
            name: BASE.into(),
            dir: root.join("base"),
            info: None,
        }];
        for name in order {
            let dir = root.join("mods").join(name);
            let file = format!("mods/{name}/mod.toml");
            let text = std::fs::read_to_string(dir.join("mod.toml"))
                .map_err(|e| LayerError::Io(file.clone(), e.to_string()))?;
            let info = ModInfo::from_toml(&file, &text)?;
            if info.native_format_version != FORMAT_VERSION {
                return Err(bad(
                    &file,
                    format!(
                        "native_format_version {} is not known (this build reads {FORMAT_VERSION})",
                        info.native_format_version
                    ),
                ));
            }
            for r in &info.requires {
                if !layers.iter().any(|l| &l.name == r) {
                    return Err(bad(
                        &file,
                        format!("requires `{r}`, which is not enabled before it in {ORDER_FILE}"),
                    ));
                }
            }
            let files_dir = dir.join("files");
            let files = if files_dir.is_dir() {
                walk(&files_dir)?
            } else {
                Vec::new()
            };
            check_mod_files(name, &files)?;
            layers.push(Layer {
                name: name.clone(),
                dir: files_dir,
                info: Some(info),
            });
        }
        Ok(LayerStack { layers })
    }

    /// The last layer holding the native file `path` (later wins, §6 r2).
    pub fn find(&self, path: &str) -> Option<&Layer> {
        self.layers
            .iter()
            .rev()
            .find(|l| l.dir.join(path).is_file())
    }

    /// Reads `path` from layer `layer` only (a pair's other half comes
    /// from the same layer, §6 r2).
    pub fn read_in(&self, layer: &Layer, path: &str) -> Option<Result<Vec<u8>, LayerError>> {
        let p = layer.dir.join(path);
        match std::fs::read(&p) {
            Ok(b) => Some(Ok(b)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => Some(Err(io(&p, e))),
        }
    }

    /// Reads `path` from the last layer that has it.
    pub fn read(&self, path: &str) -> Option<Result<Vec<u8>, LayerError>> {
        self.find(path).and_then(|l| self.read_in(l, path))
    }

    /// Every mod's table patches, in stack order: layer order, then file
    /// name (byte order) within a layer (§6 r3). `(stack path, bytes)`.
    pub fn patches(&self) -> Result<Vec<(String, Vec<u8>)>, LayerError> {
        let mut out = Vec::new();
        for l in self.layers.iter().skip(1) {
            let dir = l.dir.join(PATCH_DIR);
            if !dir.is_dir() {
                continue;
            }
            for f in walk(&dir)? {
                let p = dir.join(&f);
                let bytes = std::fs::read(&p).map_err(|e| io(&p, e))?;
                out.push((format!("mods/{}/files/{PATCH_DIR}/{f}", l.name), bytes));
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn files(list: &[&str]) -> Vec<String> {
        let mut v: Vec<String> = list.iter().map(|s| s.to_string()).collect();
        v.sort();
        v
    }

    // Covers: specs/formats/native-assets.md §6
    #[test]
    fn mod_excel_and_tbl_copies_are_refused() {
        for f in [
            "data/global/excel/weapons.txt",
            "data/local/lng/eng/string.tbl.tsv",
        ] {
            let e = check_mod_files("m", &files(&[f])).unwrap_err();
            assert!(e.to_string().contains(f), "{e}");
        }
        check_mod_files("m", &files(&["patches/a.d2patch"])).unwrap();
    }

    // Covers: specs/formats/native-assets.md §6
    #[test]
    fn half_pairs_are_refused() {
        let e = check_mod_files("m", &files(&["data/x.dc6.toml"])).unwrap_err();
        assert!(e.to_string().contains("data/x.dc6.png"), "{e}");
        let e = check_mod_files("m", &files(&["data/x.dcc.png"])).unwrap_err();
        assert!(e.to_string().contains("data/x.dcc.toml"), "{e}");
        let e = check_mod_files("m", &files(&["data/t.dt1.toml"])).unwrap_err();
        assert!(e.to_string().contains("data/t.dt1.d/"), "{e}");
        let e = check_mod_files("m", &files(&["data/t.dt1.d/0.png"])).unwrap_err();
        assert!(e.to_string().contains("data/t.dt1.toml"), "{e}");
        check_mod_files(
            "m",
            &files(&[
                "data/x.dc6.toml",
                "data/x.dc6.png",
                "data/t.dt1.toml",
                "data/t.dt1.d/0.png",
            ]),
        )
        .unwrap();
    }

    // Covers: specs/formats/native-assets.md §6
    #[test]
    fn order_and_mod_toml_are_strict() {
        assert_eq!(
            parse_order("order = [\"a\", \"b\"]").unwrap(),
            vec!["a".to_string(), "b".to_string()]
        );
        assert!(parse_order("order = [\"a\", \"a\"]").is_err());
        assert!(parse_order("order = 3").is_err());
        let ok = "name = \"a\"\nversion = \"1\"\nnative_format_version = 1\n";
        assert!(ModInfo::from_toml("m", ok).unwrap().requires.is_empty());
        assert!(ModInfo::from_toml("m", &format!("{ok}extra = 1\n")).is_err());
        assert!(ModInfo::from_toml("m", "name = \"a\"\n").is_err());
    }
}
