// Spec: specs/client/assets.md (§A1–§A5), specs/render/unit-composite.md (§2, §2.1, §3 r1, r3, §5.1, §6), specs/client/model.md (§8 r4), specs/sim/pathing.md (§8.3, §10 r2)
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
//! A COF that fails to load is skipped with a log line, never retried; a
//! component file that is in no archive is the normal empty slot of
//! `unit-composite.md` §6 r4 (e.g. a player's SH `lit` with no shield,
//! §5.1 r3): remembered, no log line. A component file that is there but
//! does not parse is logged.
//!
//! Direction (§3 r1): the model holds no client path record, so the
//! facing [`UnitArt::dir64`] reads is a preview fill (`// d2rs-own,
//! unverified`, PROVISIONAL until REC-51 records the client's turns):
//! the local player's predicted facing ([`UnitArt::pose_dir`], from
//! `bridge::predict`); for other units the `sim/pathing.md` §8.3
//! direction toward the target of a player's walk / run mode request
//! (`client/model.md` §8 r4 codes 0x01 / 0x17 to a point, 0x00 / 0x18 to
//! a unit; `sim/pathing.md` §10 r2), else toward the unit's last position
//! change ([`UnitArt::observe_facing`]); 0 when none is known.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, RwLock};

use d2_data::bin::{excel_path, BinTable};
use d2_data::tables::{
    decode_all, Composit, Monmode, Monstats, Monstats2, Objects, Objmode, Plrmode, Plrtype, Record,
    States,
};
use d2_formats::cof::{Cof, CofLayer};

use crate::assets::path::{CanonicalPath, FileSource};
use crate::bridge::predict::{cell_centre, facing};
use crate::bridge::world::{ClientWorld, UnitKey, PLAYER};
use crate::bridge::ClientUnit;
use crate::frames::{FramePart, FrameSet, FrameSetKey};
use crate::rules::unit_composite::{
    armor_class, code, expected_directions, file_format, mode_overrides, mode_token,
    monster_weapon_class, ArmorSource, Code, CofName, ComponentCodes, CompositeKind,
    DirectionSource, FileFormat, MonsterLook, PlayerLook, EMPTY, HTH,
};
use crate::rules::unit_visibility::CelBox;

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
    /// `objects` `BlocksLight0`–`7` by row: an object casts its composite
    /// shadow only in a mode whose value is ≠ 0 (`blend-modes.md` §5 r3
    /// revision, REC-518). A row not here casts it.
    pub object_blocks_light: BTreeMap<u32, [u8; 8]>,
    /// The `objects` rows whose `Draw` (`+0x150`) is 0: the unit draw
    /// `0x00471EC0` returns before any cel or shadow (`unit-composite.md`
    /// §8).
    pub object_no_draw: std::collections::BTreeSet<u32>,
    /// The shape states' draw identity (`unit-composite.md` §1.1).
    pub shapes: super::disguise::Disguise,
    /// By `monstats` row: bit v set when choice v of the `SH` component
    /// (slot 7) is a usable shield (`combat/hit.md` §5: not `tch `, and
    /// its items record has type 2; `0x006225F0`).
    pub shield_choices: BTreeMap<u32, u16>,
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

/// The `monstats2` mode bits (mDT = bit 0 … mRN = bit 15) by monster row.
fn monstats2_modes(monstats: &[Monstats], monstats2: &[Monstats2]) -> BTreeMap<u32, u16> {
    monstats
        .iter()
        .enumerate()
        .filter_map(|(i, m)| {
            let r = monstats2.get(usize::from(m.monstatsex))?;
            let bits = [
                r.mdt, r.mnu, r.mwl, r.mgh, r.ma1, r.ma2, r.mbl, r.msc, r.ms1, r.ms2, r.ms3, r.ms4,
                r.mdd, r.mkb, r.msq, r.mrn,
            ]
            .iter()
            .enumerate()
            .fold(0u16, |a, (b, &on)| a | (u16::from(on) << b));
            Some((i as u32, bits))
        })
        .collect()
}

fn code4(c: [u8; 4]) -> Code {
    // `.bin` codes are zero padded; the composite codes are space padded
    // (`0x004DA720`).
    let n = c.iter().position(|&b| b == 0).unwrap_or(4);
    code(&c[..n])
}

/// `SH` choice bits by `monstats` row (see [`UnitLooks::shield_choices`]):
/// choice byte v at record offset 38 + 12·7 + v of the `monstats2` row
/// indexes `compcode`; the code is looked up in the item tables
/// (`0x00633640`) and its `type` read.
fn shield_choices(
    source: &dyn FileSource,
    monstats2: &[Monstats2],
) -> Result<BTreeMap<u32, u16>, String> {
    let file = excel_path("monstats2.bin");
    let bytes = source
        .read_file(&file)
        .ok_or_else(|| format!("{file}: in no archive"))??;
    let raw = BinTable::parse("monstats2", "archive", &file, &bytes, Monstats2::SIZE)
        .map_err(|e| e.to_string())?;
    let comp: Vec<[u8; 4]> = read_table::<d2_data::tables::Compcode>(source)?
        .iter()
        .map(|r| r.code)
        .collect();
    let mut shield_codes: Vec<[u8; 4]> = Vec::new();
    for (code, ty) in read_table::<d2_data::tables::Armor>(source)?
        .iter()
        .map(|r| (r.code, r.type_))
        .chain(
            read_table::<d2_data::tables::Weapons>(source)?
                .iter()
                .map(|r| (r.code, r.type_)),
        )
        .chain(
            read_table::<d2_data::tables::Misc>(source)?
                .iter()
                .map(|r| (r.code, r.type_)),
        )
    {
        if ty == 2 {
            shield_codes.push(code);
        }
    }
    let mut out = BTreeMap::new();
    for (i, m) in read_table::<Monstats>(source)?.iter().enumerate() {
        let row = usize::from(m.monstatsex);
        if row >= monstats2.len() || row >= raw.count {
            continue;
        }
        let rec = raw.record(row);
        let mut mask = 0u16;
        for v in 0..12 {
            let c = usize::from(rec[38 + 12 * 7 + v]);
            let Some(code) = comp.get(c) else { continue };
            // Table 0x00744580 holds one code, `tch `.
            if code == b"tch\0" || code == b"tch " {
                continue;
            }
            if shield_codes.contains(code) {
                mask |= 1 << v;
            }
        }
        if mask != 0 {
            out.insert(i as u32, mask);
        }
    }
    Ok(out)
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
        let shapes = super::disguise::Disguise {
            states: read_table::<States>(source)?
                .iter()
                .enumerate()
                .filter(|(_, r)| matches!(r.gfxtype, 1 | 2))
                .map(|(i, r)| (i as u8, r.gfxtype, r.gfxclass))
                .collect(),
            mode_bits: monstats2_modes(&read_table::<Monstats>(source)?, &monstats2),
        };
        let object_rows = read_table::<Objects>(source)?;
        let objects = object_rows
            .iter()
            .enumerate()
            .map(|(i, o)| (i as u32, code4([o.token[0], o.token[1], 0, 0])))
            .collect();
        let object_blocks_light = object_rows
            .iter()
            .enumerate()
            .map(|(i, o)| {
                (
                    i as u32,
                    [
                        o.blockslight0,
                        o.blockslight1,
                        o.blockslight2,
                        o.blockslight3,
                        o.blockslight4,
                        o.blockslight5,
                        o.blockslight6,
                        o.blockslight7,
                    ],
                )
            })
            .collect();
        let object_no_draw = object_rows
            .iter()
            .enumerate()
            .filter(|(_, o)| o.draw == 0)
            .map(|(i, _)| i as u32)
            .collect();
        let shield_choices = shield_choices(source, &monstats2)?;
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
            object_blocks_light,
            object_no_draw,
            shapes,
            shield_choices,
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
    let unit = &*looks.shapes.identity(unit);
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
    let unit = &*looks.shapes.identity(unit);
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
        layer_weapon_class(layer).unwrap_or(name.weapon_class),
    )
}

/// The weapon class of a component file name (§6 r1): the COF layer's
/// own (`weaponClass`, nul-padded in the file), not the unit's §2.1 class.
/// PROVISIONAL (REC-441, measured: 1.14d's `a1-town-arrival-ama` draws
/// `amTNhth.cof`'s layers as `AMTRlitTN1ht`, ..., `AMRAlitTNhth`, each
/// with its layer's class). An empty layer class keeps the unit's.
fn layer_weapon_class(layer: &CofLayer) -> Option<Code> {
    let mut c = layer.weapon_class;
    for b in &mut c {
        if *b == 0 {
            *b = b' ';
        }
    }
    (c != *b"    ").then_some(c)
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
    /// The local player's predicted facing `dir64` (`Predict::facing`),
    /// set with [`Self::pose_mode`]. d2rs-own, unverified.
    pub pose_dir: Option<(UnitKey, u8)>,
    /// The unit in mode 18 and the sequence frame it draws
    /// (`world_view::skill_motion`, `skills/sequences.md` §3: the drawn
    /// mode goes in [`Self::pose_mode`]).
    pub sequence: Option<(UnitKey, usize)>,
    /// The server tick the local player's predicted walk started on
    /// (`Predict::walk_since`): its walk frames count from there
    /// (`sim/units.md` §4.7 step 7 revision, REC-516).
    /// With the animation speed (8.8 per tick) of the predicted mode.
    pub pose_since: Option<(UnitKey, u64, u64)>,
    /// The model facing of every unit (module doc), by
    /// [`Self::observe_facing`].
    pub facing: BTreeMap<UnitKey, Facing>,
    /// The model's local player at the last [`Self::observe_facing`].
    pub local: Option<UnitKey>,
    /// The cel fields of every loaded component file, by file direction
    /// and frame (`render/sprite-placement.md` §3): what the visibility
    /// predicate's cel box test reads (`super::visibility`).
    pub cels: BTreeMap<CanonicalPath, Vec<Vec<CelBox>>>,
}

/// A unit's facing as the view last saw it (module doc).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Facing {
    /// The model position at the last observation.
    pub at: (u16, u16),
    /// `dir64` (0–63).
    pub dir64: u8,
}

/// The cell a player's walk / run mode request goes to (`client/model.md`
/// §8 r4: codes 0x01 / 0x17 to the point (r0, r1), 0x00 / 0x18 to the
/// unit (type r0, GUID r1); `sim/pathing.md` §10 r2).
fn request_target(world: &ClientWorld, unit: &ClientUnit) -> Option<(u16, u16)> {
    if unit.key.unit_type != PLAYER {
        return None;
    }
    let r = unit.last_mode_request.as_ref()?;
    match r.code {
        0x01 | 0x17 => Some((
            u16::try_from(r.record[0]).ok()?,
            u16::try_from(r.record[1]).ok()?,
        )),
        0x00 | 0x18 => {
            let key = UnitKey::new(u8::try_from(r.record[0]).ok()?, r.record[1] as u32);
            world.units.get(&key)?.position
        }
        _ => None,
    }
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
}

impl UnitArt {
    /// Updates [`Self::facing`] from the model (module doc; d2rs-own,
    /// unverified): a C monster's path direction (`ClientWorld::view_direction`,
    /// the 1.14d unit's own); else toward a player's walk target when it has one and is
    /// not on it, else toward the last position change; otherwise the
    /// facing stays. Units no longer in the model are dropped.
    pub fn observe_facing(&mut self, world: &ClientWorld) {
        self.local = world.local_player;
        self.facing.retain(|k, _| world.view_unit(k).is_some());
        for unit in world.view_units() {
            let unit = &*unit;
            let Some(pos) = unit.position else { continue };
            let old = self.facing.get(&unit.key).copied();
            let toward = |to: (u16, u16)| facing(cell_centre(pos), cell_centre(to));
            let dir = world
                .view_direction(&unit.key)
                .or_else(|| request_target(world, unit).and_then(toward))
                .or_else(|| old.and_then(|f| facing(cell_centre(f.at), cell_centre(pos))))
                .or(old.map(|f| f.dir64))
                .unwrap_or(0);
            self.facing.insert(
                unit.key,
                Facing {
                    at: pos,
                    dir64: dir,
                },
            );
        }
    }

    /// The `dir64` the view draws `unit` with (module doc): the predicted
    /// facing of the local player, else the observed facing, else 0.
    pub fn dir64(&self, unit: &ClientUnit) -> u8 {
        match self.pose_dir {
            Some((key, d)) if key == unit.key => d,
            _ => self.facing.get(&unit.key).map_or(0, |f| f.dir64),
        }
    }

    /// §3 r3 expected direction count `n` of `unit` drawn with a COF of
    /// `d` directions: players 8, the local player 16 (`[0x007A8928]` = 0
    /// on a full install, open question 8); objects 1. Monsters: `d`
    /// (d2rs-own, unverified: `monstats2` `d<mode>` is not in
    /// [`UnitLooks`]).
    pub fn expected_directions(&self, unit: &ClientUnit, kind: CompositeKind, d: u8) -> u8 {
        let mode = i32::try_from(unit.mode).unwrap_or(-1);
        match kind {
            CompositeKind::Player => expected_directions(
                DirectionSource::Player {
                    sixteen: self.local == Some(unit.key),
                },
                mode,
            ),
            CompositeKind::Monster => d,
            CompositeKind::Object => expected_directions(DirectionSource::Other, mode),
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
        art.observe_facing(world);
        for unit in world.view_units() {
            let unit = &*unit;
            let posed = art.posed(unit).into_owned();
            let unit = &*self.looks.shapes.identity(&posed);
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
                // §6 r4: a file in no archive is the normal empty slot.
                if let Err(Some(e)) = &loaded {
                    log.push(format!("unit art: {}: {e}", codes.file(format)));
                }
                let loaded = loaded.ok().map(|(path, facts, cels)| {
                    art.cels.insert(path.clone(), cels);
                    (path, facts)
                });
                art.files.insert(key, loaded);
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
    /// `Err(None)`: the file is in no archive (§6 r4, not an error).
    fn load_file(
        &self,
        codes: &ComponentCodes,
        format: FileFormat,
        assets: &mut ViewAssets,
    ) -> Result<StoredFile, Option<String>> {
        let file = codes.file(format);
        let src = self.source.as_ref();
        let loaded = match format {
            FileFormat::Dcc => {
                crate::assets::path::read_dcc(src, &file).map(|r| r.map(Loaded::Dcc))
            }
            FileFormat::Dc6 => {
                crate::assets::path::read_dc6(src, &file).map(|r| r.map(Loaded::Dc6))
            }
        };
        let loaded = match loaded {
            None => return Err(None),
            Some(r) => r.map_err(Some)?,
        };
        self.store_file(codes, format, loaded, assets).map_err(Some)
    }

    fn store_file(
        &self,
        codes: &ComponentCodes,
        format: FileFormat,
        loaded: Loaded,
        assets: &mut ViewAssets,
    ) -> Result<StoredFile, String> {
        let path = codes.path(format).map_err(|e| e.to_string())?;
        let mut sets = Vec::new();
        let cels = cel_boxes(&loaded);
        let (directions, frames) = match format {
            FileFormat::Dcc => {
                let Loaded::Dcc(dcc) = loaded else {
                    return Err("format mismatch".into());
                };
                let d = u8::try_from(dcc.directions.len()).map_err(|_| "too many directions")?;
                for dir in 0..d {
                    let set = FrameSet::from_dcc(&dcc, dir).map_err(|e| e.to_string())?;
                    sets.push(sprite_cache_limit(set));
                }
                (d, dcc.frames_per_direction as usize)
            }
            FileFormat::Dc6 => {
                let Loaded::Dc6(dc6) = loaded else {
                    return Err("format mismatch".into());
                };
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
                // The derived shadow set (`unit_shadow`), frame for frame.
                let shadow = super::unit_shadow::shadow_key(&key)?;
                let shadows = super::unit_shadow::shadow_set(&set);
                assets.frames.insert(key, set).map_err(|e| e.to_string())?;
                assets
                    .frames
                    .insert(shadow, shadows)
                    .map_err(|e| e.to_string())?;
            }
        }
        Ok((
            path,
            FileFacts {
                format,
                directions,
                frames,
            },
            cels,
        ))
    }
}

/// Largest DCC frame side the sprite cache draws
/// (`render/sprite-placement.md` §3, `formats/dcc.md` §Frame size limit).
pub const SPRITE_CACHE_SIDE: u32 = 256;

/// A component DCC direction as the sprite cache gives it
/// (`render/sprite-placement.md` §3, PROVISIONAL): a frame wider or taller
/// than [`SPRITE_CACHE_SIDE`] has no cel, so it draws nothing; the frame
/// stays in the set as an empty image, keeping every other index.
pub fn sprite_cache_limit(mut set: FrameSet) -> FrameSet {
    for f in &mut set.frames {
        if f.width > SPRITE_CACHE_SIDE || f.height > SPRITE_CACHE_SIDE {
            f.width = 0;
            f.height = 0;
            f.pixels = Vec::new();
        }
    }
    set
}

/// The cel fields w, h, xoff, yoff of every frame (`render/sprite-placement.md`
/// §3): a DC6 frame header as stored; a DCC frame header's width, height
/// and x / y offset, unchanged.
fn cel_boxes(loaded: &Loaded) -> Vec<Vec<CelBox>> {
    let i = |v: u32| i32::try_from(v).unwrap_or(i32::MAX);
    match loaded {
        Loaded::Dcc(dcc) => dcc
            .directions
            .iter()
            .map(|d| {
                d.frames
                    .iter()
                    .map(|f| CelBox {
                        w: i(f.width),
                        h: i(f.height),
                        ox: f.x_offset,
                        oy: f.y_offset,
                    })
                    .collect()
            })
            .collect(),
        Loaded::Dc6(dc6) => (0..dc6.header.directions as usize)
            .map(|d| {
                (0..dc6.header.frames_per_direction as usize)
                    .map_while(|f| dc6.frame(d, f))
                    .map(|f| CelBox {
                        w: i(f.width),
                        h: i(f.height),
                        ox: f.offset_x,
                        oy: f.offset_y,
                    })
                    .collect()
            })
            .collect(),
    }
}

/// A stored component file: its path, facts and cel fields by direction
/// and frame.
type StoredFile = (CanonicalPath, FileFacts, Vec<Vec<CelBox>>);

/// A unit art file read through [`FileSource::read_native`].
enum Loaded {
    Dcc(d2_formats::dcc::Dcc),
    Dc6(d2_formats::dc6::Dc6),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frames::IndexFrame;

    fn frame(w: u32, h: u32) -> IndexFrame {
        IndexFrame::new(w, h, -4, -7, vec![1; (w * h) as usize]).unwrap()
    }

    // render/sprite-placement.md §3 (PROVISIONAL): a DCC frame with w > 256
    // or h > 256 draws nothing; frames up to 256 × 256 draw unchanged.
    #[test]
    fn sprite_cache_drops_frames_over_256() {
        let set = FrameSet {
            frames: vec![frame(256, 256), frame(257, 1), frame(1, 257), frame(3, 2)],
        };
        let out = sprite_cache_limit(set.clone());
        assert_eq!(out.frames[0], set.frames[0]);
        assert!(out.frames[1].is_empty() && out.frames[1].pixels.is_empty());
        assert!(out.frames[2].is_empty() && out.frames[2].pixels.is_empty());
        assert_eq!(out.frames[3], set.frames[3]);
    }
}
