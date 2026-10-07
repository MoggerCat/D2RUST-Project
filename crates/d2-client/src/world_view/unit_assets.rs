// Spec: specs/client/assets.md (§A1–§A5), specs/render/unit-composite.md (§2, §2.1, §5.1, §6)
//! Unit art for the play preview: the table facts a unit composite reads
//! ([`UnitLooks`]: unit tokens, mode tokens, component tokens, monster
//! base weapon class), the COF and component file names of a model unit
//! ([`unit_cof`], [`component_codes`]), and the loader that makes those
//! files resident from the user's archives ([`UnitArtLoader`]): the COF
//! into [`ViewAssets::cofs`], each direction of a component file into
//! [`ViewAssets::frames`], and the file facts the rules read
//! ([`UnitArt`], shared with [`super::unit_rules::UnitRules`]).
//!
//! Inputs the model lacks get the D1 preview fills (`docs/PLAN.md`
//! decisions, "First playable preview"), each marked
//! `d2rs-own, unverified`: a player has no items (every armor class
//! `lit`, weapon class `hth`), a monster's component choices are all 0.
//! A file that fails to load is skipped with a log line, never retried.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, RwLock};

use d2_data::bin::{excel_path, BinTable};
use d2_data::tables::{
    decode_all, Composit, Monmode, Monstats, Monstats2, Objects, Objmode, Plrmode, Plrtype, Record,
};
use d2_formats::cof::{Cof, CofLayer};

use crate::assets::path::{CanonicalPath, FileSource};
use crate::bridge::world::{ClientWorld, UnitKey};
use crate::bridge::ClientUnit;
use crate::frames::{FramePart, FrameSet, FrameSetKey};
use crate::rules::unit_composite::{
    armor_class, code, file_format, mode_overrides, mode_token, monster_weapon_class, ArmorSource,
    Code, CofName, ComponentCodes, CompositeKind, FileFormat, MonsterLook, PlayerLook, EMPTY, HTH,
};

use super::ViewAssets;

/// The `monstats` / `monstats2` facts of one monster class.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MonsterRow {
    /// `monstats` `Code` (§5.1 unit token).
    pub token: Code,
    /// `monstats2` `BaseW` (§2.1); `None`: no `monstats2` row.
    pub base_w: Option<Code>,
    /// `monstats2` `compositeDeath`.
    pub composite_death: bool,
}

/// The table facts of §2 / §5.1 for every unit kind.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UnitLooks {
    /// `plrtype` token by player class.
    pub player_tokens: Vec<Code>,
    /// `plrmode`, `monmode`, `objmode` token by mode.
    pub player_modes: Vec<Code>,
    pub monster_modes: Vec<Code>,
    pub object_modes: Vec<Code>,
    /// `composit` token by component id (rows 0–15).
    pub components: Vec<Code>,
    /// By `monstats` row.
    pub monsters: BTreeMap<u32, MonsterRow>,
    /// `objects` `Token` by row.
    pub objects: BTreeMap<u32, Code>,
}

fn read_table<T: Record>(source: &dyn FileSource) -> Result<Vec<T>, String> {
    let file = excel_path(&format!("{}.bin", T::TABLE));
    let bytes = source
        .read_file(&file)
        .ok_or_else(|| format!("{file}: in no archive"))??;
    let table =
        BinTable::parse(T::TABLE, "archive", &file, &bytes, T::SIZE).map_err(|e| e.to_string())?;
    decode_all(&table).map_err(|e| e.to_string())
}

fn code4(c: [u8; 4]) -> Code {
    // `.bin` codes are zero padded; the composite codes are space padded
    // (`0x004DA720`).
    let n = c.iter().position(|&b| b == 0).unwrap_or(4);
    code(&c[..n])
}

impl UnitLooks {
    /// The facts from the user's `.bin` tables (`data\global\excel`).
    pub fn live(source: &dyn FileSource) -> Result<Self, String> {
        let monstats2: Vec<Monstats2> = read_table(source)?;
        let monsters = read_table::<Monstats>(source)?
            .iter()
            .enumerate()
            .map(|(i, m)| {
                let row = monstats2.get(usize::from(m.monstatsex));
                let base_w = row.map(|r| code4(r.basew)).filter(|w| *w != code(b""));
                (
                    i as u32,
                    MonsterRow {
                        token: code4(m.code),
                        base_w,
                        composite_death: row.is_some_and(|r| r.compositedeath),
                    },
                )
            })
            .collect();
        let objects = read_table::<Objects>(source)?
            .iter()
            .enumerate()
            .map(|(i, o)| (i as u32, code4([o.token[0], o.token[1], 0, 0])))
            .collect();
        Ok(UnitLooks {
            player_tokens: read_table::<Plrtype>(source)?
                .iter()
                .map(|r| code4(r.token))
                .collect(),
            player_modes: read_table::<Plrmode>(source)?
                .iter()
                .map(|r| code4(r.token))
                .collect(),
            monster_modes: read_table::<Monmode>(source)?
                .iter()
                .map(|r| code4(r.token))
                .collect(),
            object_modes: read_table::<Objmode>(source)?
                .iter()
                .map(|r| code4(r.token))
                .collect(),
            components: read_table::<Composit>(source)?
                .iter()
                .map(|r| code4(r.token))
                .collect(),
            monsters,
            objects,
        })
    }

    fn modes(&self, kind: CompositeKind) -> &[Code] {
        match kind {
            CompositeKind::Player => &self.player_modes,
            CompositeKind::Monster => &self.monster_modes,
            CompositeKind::Object => &self.object_modes,
        }
    }
}

/// The unit's composite kind and the `u8` mode the rules read.
fn kind_mode(unit: &ClientUnit) -> Option<(CompositeKind, u8)> {
    let kind = CompositeKind::of_unit_type(u32::from(unit.key.unit_type))?;
    Some((kind, u8::try_from(unit.mode).ok()?))
}

/// §2.1 weapon class. Players: `hth` (D1 preview fill: the model holds no
/// equipped items; `// d2rs-own, unverified`). Objects: `hth`.
fn weapon_class(looks: &UnitLooks, unit: &ClientUnit, kind: CompositeKind, mode: u8) -> Code {
    match kind {
        CompositeKind::Monster => {
            let row = looks.monsters.get(&unit.class);
            monster_weapon_class(
                mode,
                row.is_some_and(|r| r.composite_death),
                row.and_then(|r| r.base_w),
            )
        }
        // d2rs-own, unverified (D1): no items, bare hands.
        CompositeKind::Player | CompositeKind::Object => HTH,
    }
}

/// §5.1 unit token.
fn unit_token(looks: &UnitLooks, unit: &ClientUnit, kind: CompositeKind) -> Option<Code> {
    let t = match kind {
        CompositeKind::Player => looks.player_tokens.get(unit.class as usize).copied(),
        CompositeKind::Monster => looks.monsters.get(&unit.class).map(|r| r.token),
        CompositeKind::Object => looks.objects.get(&unit.class).copied(),
    }?;
    (t != code(b"") && t != EMPTY).then_some(t)
}

/// The mode token of §5.1 (empty: none), before the §2 r2 override.
fn base_mode_token(looks: &UnitLooks, kind: CompositeKind, mode: u8) -> Code {
    looks
        .modes(kind)
        .get(usize::from(mode))
        .copied()
        .unwrap_or(EMPTY)
}

/// The COF name of a model unit (§2), `None` for a unit without a
/// composite (types 3–5, unknown class or mode).
pub fn unit_cof(looks: &UnitLooks, unit: &ClientUnit) -> Option<CofName> {
    let (kind, mode) = kind_mode(unit)?;
    let token = unit_token(looks, unit, kind)?;
    let m = mode_token(
        base_mode_token(looks, kind, mode),
        mode,
        mode_overrides(kind),
    );
    if m == EMPTY {
        return None;
    }
    let w = weapon_class(looks, unit, kind, mode);
    Some(match kind {
        CompositeKind::Player => CofName::player(token, mode, m, w),
        _ => CofName {
            kind,
            token,
            mode_token: m,
            weapon_class: w,
        },
    })
}

/// The §5.1 request of the COF layer `layer`, `None` when it fails (the
/// slot draws nothing). D1 preview fills (`// d2rs-own, unverified`): a
/// player has no items and no body armor; a monster's choices are all 0
/// with no `monstats2` counts (every component `lit`).
pub fn component_codes(
    looks: &UnitLooks,
    unit: &ClientUnit,
    name: &CofName,
    layer: &CofLayer,
) -> Option<ComponentCodes> {
    let (kind, mode) = kind_mode(unit)?;
    let c = layer.component;
    let component_token = *looks.components.get(usize::from(c))?;
    let player = PlayerLook {
        body_armor: None,
        item_gfx: [None; 16],
        holy_shield: false,
        shield_hand_item: false,
    };
    let monster = MonsterLook {
        choices: [0; 16],
        counts: [0; 16],
        codes: [[EMPTY; 12]; 16],
        composite_death: looks
            .monsters
            .get(&unit.class)
            .is_some_and(|r| r.composite_death),
        act_two: false,
        base_class: unit.class,
    };
    let source = match kind {
        CompositeKind::Player => ArmorSource::Player(Some(&player)),
        CompositeKind::Monster => ArmorSource::Monster(&monster),
        CompositeKind::Object => ArmorSource::Object,
    };
    let armor = armor_class(source, c, mode, name.weapon_class).ok()?;
    ComponentCodes::new(
        kind,
        name.token,
        component_token,
        armor,
        base_mode_token(looks, kind, mode),
        name.weapon_class,
    )
}

/// One loaded component file: its format, directions `Df` and frames per
/// direction `Ff` (§6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileFacts {
    pub format: FileFormat,
    pub directions: u8,
    pub frames: usize,
}

/// What the loader made resident, read by the rules (they get no
/// [`ViewAssets`] in `unit_pose` / `component_slot_frame`).
#[derive(Debug, Clone, Default)]
pub struct UnitArt {
    /// Loaded COFs (also in [`ViewAssets::cofs`]).
    pub cofs: BTreeMap<CanonicalPath, Cof>,
    /// Component files by request name (§6 r1, no extension): `None` =
    /// tried and missing or refused (the slot draws nothing).
    pub files: BTreeMap<String, Option<(CanonicalPath, FileFacts)>>,
    /// COF paths tried and missing or refused.
    pub missing_cofs: BTreeSet<CanonicalPath>,
    /// The mode the local player is drawn in while the play preview's
    /// walk prediction moves it (decision D2, `bridge::predict`: 2 walk,
    /// 3 run). d2rs-own, unverified.
    pub pose_mode: Option<(UnitKey, u32)>,
    /// The direction (`dir64`, 0–63) the local player is drawn facing in
    /// the play preview: the facing of its predicted movement
    /// (`bridge::predict::Predict::facing`), kept after it stops.
    /// d2rs-own, unverified. PROVISIONAL (client/model.md OQ2; REC-51).
    /// Only `play` sets it; the strict path never does.
    pub pose_dir: Option<(UnitKey, u8)>,
}

impl UnitArt {
    /// `unit` with its drawn mode ([`Self::pose_mode`] for its key, else
    /// the model's).
    pub fn posed<'a>(&self, unit: &'a ClientUnit) -> std::borrow::Cow<'a, ClientUnit> {
        match self.pose_mode {
            Some((key, mode)) if key == unit.key && unit.mode != mode => {
                let mut u = unit.clone();
                u.mode = mode;
                std::borrow::Cow::Owned(u)
            }
            _ => std::borrow::Cow::Borrowed(unit),
        }
    }

    /// `dir64` of `unit` (`render/unit-composite.md` §3 r1):
    /// [`Self::pose_dir`] for its key, else 0 (d2rs-own, unverified: the
    /// model holds no client path record).
    pub fn dir64(&self, unit: &ClientUnit) -> u8 {
        match self.pose_dir {
            Some((key, d)) if key == unit.key => d & 63,
            _ => 0,
        }
    }
}

pub type SharedUnitArt = Arc<RwLock<UnitArt>>;

/// Reads the unit files the model's units name, once each.
pub struct UnitArtLoader {
    pub source: Arc<dyn FileSource>,
    pub looks: Arc<UnitLooks>,
    pub art: SharedUnitArt,
}

impl UnitArtLoader {
    /// Makes the COF and component files of every unit in `world`
    /// resident. D1 (`// d2rs-own, unverified`): a file that fails is
    /// skipped and remembered; returns one log line per new failure.
    pub fn ensure(&self, world: &ClientWorld, assets: &mut ViewAssets) -> Vec<String> {
        let mut log = Vec::new();
        let mut art = self.art.write().unwrap_or_else(|e| e.into_inner());
        for unit in world.units.values() {
            let posed = art.posed(unit).into_owned();
            let unit = &posed;
            let Some(name) = unit_cof(&self.looks, unit) else {
                continue;
            };
            let Ok(path) = name.path() else { continue };
            if art.missing_cofs.contains(&path) {
                continue;
            }
            if !art.cofs.contains_key(&path) {
                match self
                    .read(&name.full())
                    .and_then(|b| Cof::parse(&b).map_err(|e| e.to_string()))
                {
                    Ok(cof) => {
                        assets.cofs.insert(path.clone(), cof.clone());
                        art.cofs.insert(path.clone(), cof);
                    }
                    Err(e) => {
                        log.push(format!("unit art: {}: {e}", name.full()));
                        art.missing_cofs.insert(path);
                        continue;
                    }
                }
            }
            let cof = art.cofs[&path].clone();
            for layer in &cof.layers {
                let Some(codes) = component_codes(&self.looks, unit, &name, layer) else {
                    continue;
                };
                let key = codes.name();
                if art.files.contains_key(&key) {
                    continue;
                }
                let format = file_format(&codes, unit.class, unit.mode as u8);
                let loaded = self.load_file(&codes, format, assets);
                if let Err(e) = &loaded {
                    log.push(format!("unit art: {}: {e}", codes.file(format)));
                }
                art.files.insert(key, loaded.ok());
            }
        }
        log
    }

    fn read(&self, file: &str) -> Result<Vec<u8>, String> {
        self.source
            .read_file(file)
            .ok_or_else(|| "in no archive".to_string())?
    }

    /// Every direction of the file into the frame store, all or nothing.
    fn load_file(
        &self,
        codes: &ComponentCodes,
        format: FileFormat,
        assets: &mut ViewAssets,
    ) -> Result<(CanonicalPath, FileFacts), String> {
        let path = codes.path(format).map_err(|e| e.to_string())?;
        let bytes = self.read(&codes.file(format))?;
        let mut sets = Vec::new();
        let (directions, frames) = match format {
            FileFormat::Dcc => {
                let dcc = d2_formats::dcc::Dcc::parse(&bytes).map_err(|e| e.to_string())?;
                let d = u8::try_from(dcc.directions.len()).map_err(|_| "too many directions")?;
                for dir in 0..d {
                    sets.push(FrameSet::from_dcc(&dcc, dir).map_err(|e| e.to_string())?);
                }
                (d, dcc.frames_per_direction as usize)
            }
            FileFormat::Dc6 => {
                let dc6 = d2_formats::dc6::Dc6::parse(&bytes).map_err(|e| e.to_string())?;
                let d = u8::try_from(dc6.header.directions).map_err(|_| "too many directions")?;
                for dir in 0..d {
                    sets.push(FrameSet::from_dc6(&dc6, dir).map_err(|e| e.to_string())?);
                }
                (d, dc6.header.frames_per_direction as usize)
            }
        };
        for (dir, set) in sets.into_iter().enumerate() {
            let key = FrameSetKey::new(path.as_str(), FramePart::Dir(dir as u8))
                .map_err(|e| e.to_string())?;
            if !assets.frames.contains(&key) {
                assets.frames.insert(key, set).map_err(|e| e.to_string())?;
            }
        }
        Ok((
            path,
            FileFacts {
                format,
                directions,
                frames,
            },
        ))
    }
}
