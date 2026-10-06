// Spec: specs/render/unit-composite.md, specs/render/unit-directions.tsv
//! Unit composites (`render/unit-composite.md`): which COF and component
//! files a unit draws, the direction and frame indices, the component
//! request (armor class and weapon class), the colormap source per
//! component, the COF box pre-test, the per-unit extra offsets (client
//! motion record, object and missile offsets) and the single-cel units.
//! Plain Rust, integer math, no Bevy types.
//!
//! The slot loop itself (row offset, slots that draw nothing, S7) is
//! [`crate::composite`]; this module answers the questions that loop asks.
//! Inputs the client model does not hold yet (equipped items, monster
//! component choices, table rows) arrive as plain values from the caller.
//! A rule the spec leaves open is a `TODO(spec: …)` that returns
//! [`UnitCompositeError::Unresolved`], never a guess.

use d2_formats::cof::Cof;

use crate::assets::CanonicalPath;
use crate::composite::ComponentFrame;
use crate::frames::{FramePart, FrameSetKey};
use crate::world_view::UnitPose;

const SPEC: &str = "render/unit-composite.md";

/// Errors of this module: the original's fatal checks, and rules the spec
/// leaves open (open questions), which are refused rather than guessed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum UnitCompositeError {
    /// COF or file direction count not a power of two below 128 (fatal
    /// 0x1ED / 0x1EE, §3 r4).
    #[error("direction count {0} is not a power of two below 128")]
    DirectionCount(u8),
    /// A cel past `frame ≤ Ff` (fatal, §6 r3), or past the file's last cel.
    #[error(
        "cel of file direction {dir}, frame {frame} is outside the file ({directions} × {frames})"
    )]
    Cel {
        dir: u8,
        frame: usize,
        directions: u8,
        frames: usize,
    },
    /// A composed path the asset layer refuses.
    #[error("path {0:?}: {1}")]
    Path(String, String),
    /// A rule the spec leaves open (`what` names the open question).
    #[error("{what}: TODO(spec: {SPEC} {question})")]
    Unresolved {
        what: &'static str,
        question: &'static str,
    },
}

fn unresolved(what: &'static str, question: &'static str) -> UnitCompositeError {
    UnitCompositeError::Unresolved { what, question }
}

// ---------------------------------------------------------------------------
// §2, §5.1, §6 r1: 4-byte codes and names.

/// A 4-byte code as the tables store it, space-padded (`lit `, `hth `).
pub type Code = [u8; 4];

pub const LIT: Code = *b"lit ";
pub const MED: Code = *b"med ";
pub const HVY: Code = *b"hvy ";
pub const HTH: Code = *b"hth ";
pub const HT2: Code = *b"ht2 ";
pub const HSH: Code = *b"hsh ";
pub const XBW: Code = *b"xbw ";
pub const BOW: Code = *b"bow ";
/// Mode token of an empty table entry (§5.1 table).
pub const XXX: Code = *b"xxx ";
/// No code (an empty table field).
pub const EMPTY: Code = [0; 4];

/// The `armtype` tokens by index (`0x007C89C0`, §5.1 r3).
pub const ARMTYPE: [Code; 3] = [LIT, MED, HVY];

/// The name part of a code: its first 3 bytes, ending at the first space
/// or 0 (§2: every 0x20 turned into 0; §6 r1).
pub fn part(code: &Code) -> &[u8] {
    let end = code[..3]
        .iter()
        .position(|&b| b == b' ' || b == 0)
        .unwrap_or(3);
    &code[..end]
}

/// A code from a name of up to 4 bytes, space-padded (`code(b"1hs")`).
pub fn code(name: &[u8]) -> Code {
    let mut c = *b"    ";
    for (d, s) in c.iter_mut().zip(name) {
        *d = *s;
    }
    c
}

fn text(bytes: &[u8]) -> String {
    bytes.iter().map(|&b| char::from(b)).collect()
}

/// The draw path's unit kinds with a composite or a file root (§1, §2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CompositeKind {
    Player,
    Monster,
    Object,
}

impl CompositeKind {
    /// Unit types 0–2 (§1); 3, 4 draw one cel (§9), 5 nothing.
    pub fn of_unit_type(unit_type: u32) -> Option<CompositeKind> {
        match unit_type {
            0 => Some(CompositeKind::Player),
            1 => Some(CompositeKind::Monster),
            2 => Some(CompositeKind::Object),
            _ => None,
        }
    }

    /// `<root>` of §2.
    pub fn root(self) -> &'static str {
        match self {
            CompositeKind::Player => "DATA\\GLOBAL\\CHARS",
            CompositeKind::Monster => "DATA\\GLOBAL\\MONSTERS",
            CompositeKind::Object => "DATA\\GLOBAL\\OBJECTS",
        }
    }

    /// Mode DT / DD of the kind (players 0 / 17, monsters 0 / 12).
    pub fn is_death_mode(self, mode: u8) -> bool {
        match self {
            CompositeKind::Player => mode == 0 || mode == 17,
            CompositeKind::Monster => mode == 0 || mode == 12,
            CompositeKind::Object => false,
        }
    }
}

/// Player mode TH (throw, §2 r3).
pub const PLAYER_MODE_TH: u8 = 11;

/// The COF name parts of §2: unit token `T`, mode token `M` (after the
/// override tables of r2) and weapon class `W` (§2.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CofName {
    pub kind: CompositeKind,
    pub token: Code,
    pub mode_token: Code,
    pub weapon_class: Code,
}

impl CofName {
    /// §2 for a player in `mode` (r3: TH with a weapon class other than
    /// `1hs`, `1ht`, `1js`, `1jt`, `1ss`, `1st` uses `hth`).
    pub fn player(token: Code, mode: u8, mode_token: Code, weapon_class: Code) -> CofName {
        let throwable = [b"1hs", b"1ht", b"1js", b"1jt", b"1ss", b"1st"];
        let weapon_class =
            if mode == PLAYER_MODE_TH && !throwable.iter().any(|w| part(&weapon_class) == &w[..]) {
                HTH
            } else {
                weapon_class
            };
        CofName {
            kind: CompositeKind::Player,
            token,
            mode_token,
            weapon_class,
        }
    }

    /// Short form `<T><M><W>`: the AnimData key (§2 r1).
    pub fn short(&self) -> String {
        let mut s = text(part(&self.token));
        s.push_str(&text(part(&self.mode_token)));
        s.push_str(&text(part(&self.weapon_class)));
        s
    }

    /// Full form `<root>\<T>\COF\<T><M><W>.COF` (§2).
    pub fn full(&self) -> String {
        format!(
            "{}\\{}\\COF\\{}.COF",
            self.kind.root(),
            text(part(&self.token)),
            self.short()
        )
    }

    /// [`CofName::full`] as an asset path.
    pub fn path(&self) -> Result<CanonicalPath, UnitCompositeError> {
        canonical(self.full())
    }
}

fn canonical(path: String) -> Result<CanonicalPath, UnitCompositeError> {
    CanonicalPath::new(&path).map_err(|e| UnitCompositeError::Path(path, e.to_string()))
}

/// The mode override tables of §2 r2: every pair whose mode equals the
/// unit's replaces the mode token, the last match wins. Their contents are
/// runtime data (open question 1): the caller passes them; the original
/// tables are TODO(spec: render/unit-composite.md OQ1).
pub fn mode_token(token: Code, mode: u8, overrides: &[(Code, u8)]) -> Code {
    overrides
        .iter()
        .rfind(|(_, m)| *m == mode)
        .map_or(token, |(t, _)| *t)
}

// ---------------------------------------------------------------------------
// §2.1 weapon class.

/// Monster weapon class (§2.1): `hth` in DT/DD without `compositeDeath`,
/// else monstats2 BaseW; no monstats2 row: `hth`.
pub fn monster_weapon_class(mode: u8, composite_death: bool, base_w: Option<Code>) -> Code {
    if CompositeKind::Monster.is_death_mode(mode) && !composite_death {
        return HTH;
    }
    base_w.unwrap_or(HTH)
}

/// One hand item of a player as §2.1 reads it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HandItem {
    /// The item's `component` (5 RH, 6 LH count).
    pub component: u8,
    /// Item type (45 is the type of the dual-wield test).
    pub item_type: u16,
    pub wclass: Code,
    pub two_handed_wclass: Code,
    /// The grip test `0x0063D340` returned 2.
    pub two_handed_grip: bool,
}

/// What §2.1 reads for a player.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlayerHands {
    /// `charstats` class (4 Barbarian, 6 Assassin).
    pub class: u8,
    /// `charstats` weapon class (+0x4C).
    pub base_wclass: Code,
    /// Valid items in body locations 4 and 5.
    pub loc4: Option<HandItem>,
    pub loc5: Option<HandItem>,
}

/// Player weapon class for the component request (`0x0064F380`, §2.1).
/// For the COF name, modes DT/DD use `hth` instead
/// ([`player_cof_weapon_class`]).
pub fn player_weapon_class(hands: &PlayerHands) -> Result<Code, UnitCompositeError> {
    let in_hand = |i: &Option<HandItem>| i.filter(|i| i.component == 5 || i.component == 6);
    let Some(hand) = in_hand(&hands.loc4).or_else(|| in_hand(&hands.loc5)) else {
        return Ok(hands.base_wclass);
    };
    if let (Some(a), Some(b)) = (hands.loc4, hands.loc5) {
        if a.item_type == 45 && b.item_type == 45 {
            match hands.class {
                // TODO(spec: render/unit-composite.md OQ3): `1ss` / `1st` /
                // `1js` / `1jt` by the type classes of `0x00629FE0`.
                4 => return Err(unresolved("Barbarian dual-wield weapon class", "OQ3")),
                6 => return Ok(HT2),
                _ => {}
            }
        }
    }
    Ok(if hand.two_handed_grip {
        hand.two_handed_wclass
    } else {
        hand.wclass
    })
}

/// Player weapon class in the COF name (`0x0064F5B0`, §2.1): `hth` in
/// modes DT/DD, else [`player_weapon_class`].
pub fn player_cof_weapon_class(mode: u8, hands: &PlayerHands) -> Result<Code, UnitCompositeError> {
    if CompositeKind::Player.is_death_mode(mode) {
        return Ok(HTH);
    }
    player_weapon_class(hands)
}

// ---------------------------------------------------------------------------
// §3 direction and frame; tables of `unit-directions.tsv`.

/// `log2(d)` for a power-of-two direction count below 128.
fn log2(d: u8) -> Result<u32, UnitCompositeError> {
    if d == 0 || !d.is_power_of_two() || d >= 128 {
        return Err(UnitCompositeError::DirectionCount(d));
    }
    Ok(d.trailing_zeros())
}

/// COF direction of `dir64` for a COF of `d` directions (§3 r4, table
/// `0x006E55A0`): `((dir64 + 32/D) >> log2(64/D)) mod D`. `dir64` ≥ 64 is
/// direction 0 (`0x004DB2E0`).
pub fn cof_direction(d: u8, dir64: u8) -> Result<u8, UnitCompositeError> {
    let k = log2(d)?;
    if dir64 >= 64 {
        return Ok(0);
    }
    // k ≤ 6: `log2` refuses 128.
    let shift = 6 - k;
    let half = 32u32 >> k;
    Ok((((u32::from(dir64) + half) >> shift) % u32::from(d)) as u8)
}

/// `P_D` of the spec's Constants: file direction stored at angular step `k`
/// for a file of `d` directions. `P_1 = [0]`, `P_2 = [0, 0]`,
/// `P_4 = [0, 1, 2, 3]`, `P_8[2j] = 4 + j`, `P_8[2j+1] = j`,
/// `P_2D[2j] = P_D[j]`, `P_2D[2j+1] = D + j`.
fn interleave(d: u8, k: u8) -> u8 {
    match d {
        1 | 2 => 0,
        4 => k,
        8 => {
            if k.is_multiple_of(2) {
                4 + k / 2
            } else {
                k / 2
            }
        }
        _ => {
            let half = d / 2;
            if k.is_multiple_of(2) {
                interleave(half, k / 2)
            } else {
                half + k / 2
            }
        }
    }
}

/// File direction of `dir64` for a DCC/DC6 of `df` directions (§6 r3,
/// table `0x006E45A0`): `P_Df[cof_direction(Df, dir64)]`.
pub fn file_direction(df: u8, dir64: u8) -> Result<u8, UnitCompositeError> {
    let k = cof_direction(df, dir64)?;
    Ok(interleave(df, k))
}

/// Snap tables `0x006E5DA0` and `0x006E5DA8` (§3 r4).
const SNAP_8_4: [u8; 8] = [1, 1, 3, 3, 5, 5, 7, 7];
const SNAP_16_8: [u8; 16] = [0, 0, 2, 2, 4, 4, 6, 6, 8, 8, 10, 10, 12, 12, 14, 14];

/// `dir64` snapped for (COF directions `d`, expected count `n`) (§3 r4,
/// `0x004DB290`): only (8, 4) and (16, 8) snap.
pub fn snap(d: u8, n: u8, dir64: u8) -> u8 {
    match (d, n) {
        (8, 4) => SNAP_8_4[usize::from(dir64 >> 3) & 7] << 3,
        (16, 8) => SNAP_16_8[usize::from(dir64 >> 2) & 15] << 2,
        _ => dir64,
    }
}

/// Who a unit's expected direction count comes from (§3 r3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirectionSource {
    /// `sixteen`: the local player while `[0x007A8928]` = 0 (open question
    /// 8 says what sets it; the caller decides).
    Player {
        sixteen: bool,
    },
    /// monstats2 `d<mode>` (+0xF4 + mode); `mode_table_zero`: the per-mode
    /// table of `0x0046F9D0` holds 0 for the mode (open question 4).
    Monster {
        d_mode: u8,
        mode_table_zero: bool,
    },
    /// `missiles` NumDirections.
    Missile {
        num_directions: u8,
    },
    Other,
}

/// Expected direction count `n` (`0x004DAF70`, §3 r3). A mode < 0 gives 1.
pub fn expected_directions(source: DirectionSource, mode: i32) -> u8 {
    if mode < 0 {
        return 1;
    }
    match source {
        DirectionSource::Player { sixteen } => {
            if sixteen {
                16
            } else {
                8
            }
        }
        DirectionSource::Monster {
            d_mode,
            mode_table_zero,
        } => {
            if d_mode == 8 && mode_table_zero {
                4
            } else {
                d_mode
            }
        }
        DirectionSource::Missile { num_directions } => num_directions,
        DirectionSource::Other => 1,
    }
}

/// The direction answers of one drawn composite (§3 r4, r5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnitDirection {
    /// `dir64` after the snap; component files use this (§6 r3).
    pub dir64: u8,
    /// The COF row direction.
    pub cof_dir: u8,
    /// §3 r5: the value written back into the unit's path during the draw
    /// (dead player or monster with `n` ≠ `D`), else `None`.
    pub write_back: Option<u8>,
}

/// §3 r4–r5 for a unit facing `dir64` with expected count `n`, a COF of
/// `d` directions, `dead` = mode DT/DD of a player or monster.
pub fn unit_direction(
    d: u8,
    n: u8,
    dir64: u8,
    dead: bool,
) -> Result<UnitDirection, UnitCompositeError> {
    log2(d)?;
    // `0x004DB2E0`: dir64 ≥ 64 is direction 0 with n = 1.
    let (dir64, n) = if dir64 >= 64 { (0, 1) } else { (dir64, n) };
    let snapped = snap(d, n, dir64);
    Ok(UnitDirection {
        dir64: snapped,
        cof_dir: cof_direction(d, snapped)?,
        write_back: (n != d && dead).then_some(snapped),
    })
}

/// The frame index of a unit frame counter (unit +0x44, 8.8 fixed point,
/// §3 r2).
pub fn frame_index(frame_counter: u32) -> usize {
    (frame_counter >> 8) as usize
}

/// The cel of a component file (§6 r3): file direction from `dir64` (the
/// snapped value), cel `Ff × dcc_dir + frame` as a frame set of that file
/// (one set per file direction, `FramePart::Dir`). `frame = Ff` passes
/// the original's check and is the next direction's first cel; past the
/// last cel is an error (the original reads past the file).
pub fn component_cel(
    path: &CanonicalPath,
    df: u8,
    ff: usize,
    dir64: u8,
    frame: usize,
) -> Result<ComponentFrame, UnitCompositeError> {
    let dcc_dir = file_direction(df, dir64)?;
    let bad = || UnitCompositeError::Cel {
        dir: dcc_dir,
        frame,
        directions: df,
        frames: ff,
    };
    if frame > ff || ff == 0 {
        return Err(bad());
    }
    let cel = ff * usize::from(dcc_dir) + frame;
    let dir = u8::try_from(cel / ff)
        .ok()
        .filter(|&d| d < df)
        .ok_or_else(bad)?;
    let set = FrameSetKey::new(path.as_str(), FramePart::Dir(dir))
        .map_err(|e| UnitCompositeError::Path(path.to_string(), e.to_string()))?;
    Ok(ComponentFrame {
        set,
        index: cel % ff,
    })
}

/// `ComponentResolver::slot_frame` (§5 r2, §6, §10): `None` when the
/// request failed (`codes` = `None`) or the file did not load (`file` =
/// `None`), else the cel of [`component_cel`]. `file` is the loaded
/// file's (`Df`, `Ff`).
pub fn slot_frame(
    codes: Option<&ComponentCodes>,
    format: FileFormat,
    file: Option<(u8, usize)>,
    dir64: u8,
    frame: usize,
) -> Result<Option<ComponentFrame>, UnitCompositeError> {
    let (Some(codes), Some((df, ff))) = (codes, file) else {
        return Ok(None);
    };
    component_cel(&codes.path(format)?, df, ff, dir64, frame).map(Some)
}

/// `ViewRules::unit_pose` (§10): `None` unless the unit is of type 0–2
/// (`kind`), its COF loaded (`cof`, else §2 r4: not drawn) and the COF
/// box passes §4 at the final screen position `at` in a `w` × `h` frame.
/// `n` is §3 r3's expected count, `dead` the DT/DD test of §3 r5. Returns
/// the pose (COF path, `cof_dir`, frame) and the direction answers
/// (snapped `dir64` for the component files, write-back).
#[allow(clippy::too_many_arguments)]
pub fn unit_pose(
    kind: Option<CompositeKind>,
    name: &CofName,
    cof: Option<&Cof>,
    at: (i32, i32),
    size: (u32, u32),
    n: u8,
    dir64: u8,
    dead: bool,
    frame: usize,
) -> Result<Option<(UnitPose, UnitDirection)>, UnitCompositeError> {
    let (Some(_), Some(cof)) = (kind, cof) else {
        return Ok(None);
    };
    if !cof_box_visible(cof, at.0, at.1, size.0, size.1) {
        return Ok(None);
    }
    let direction = unit_direction(cof.directions, n, dir64, dead)?;
    Ok(Some((
        UnitPose {
            cof: name.path()?,
            dir: usize::from(direction.cof_dir),
            frame,
        },
        direction,
    )))
}

// ---------------------------------------------------------------------------
// §4 COF box culling.

/// §4 (`0x004709A0`): the composite at final screen position (`x`, `y`)
/// in a `w` × `h` frame is drawn only if the COF box passes all four
/// tests.
pub fn cof_box_visible(cof: &Cof, x: i32, y: i32, w: u32, h: u32) -> bool {
    let (x, y) = (i64::from(x), i64::from(y));
    let (w, h) = (i64::from(w), i64::from(h));
    i64::from(cof.x_min) + x < w - 1
        && i64::from(cof.x_max) + x >= 0
        && i64::from(cof.y_max) + y >= 0
        && i64::from(cof.y_min) + y < h - 1
}

// ---------------------------------------------------------------------------
// §5.1 component request, §6 file.

/// Component IDs used by the rules (`COMPONENT_NAMES` in `d2-formats`).
pub mod component {
    pub const HD: u8 = 0;
    pub const TR: u8 = 1;
    pub const LG: u8 = 2;
    pub const RA: u8 = 3;
    pub const LA: u8 = 4;
    pub const RH: u8 = 5;
    pub const LH: u8 = 6;
    pub const SH: u8 = 7;
    pub const S1: u8 = 8;
    pub const S2: u8 = 9;
    pub const S7: u8 = 14;
    pub const S8: u8 = 15;
}

/// The five codes of a component request (§5.1 table).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ComponentCodes {
    pub kind: CompositeKind,
    pub unit_token: Code,
    pub component_token: Code,
    pub armor_class: Code,
    pub mode_token: Code,
    pub weapon_class: Code,
}

/// The armor-class source of a request, per unit kind (§5.1 r1–r3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArmorSource<'a> {
    /// r1: `lit`.
    Object,
    Monster(&'a MonsterLook),
    /// `None`: the player has no inventory (the request fails).
    Player(Option<&'a PlayerLook>),
}

/// What §5.1 r2 reads for a monster.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MonsterLook {
    /// Client monster data +4: the choice per component.
    pub choices: [u8; 16],
    /// monstats2 counts per component (+0x15).
    pub counts: [u8; 16],
    /// `compcode` code of each component's choice bytes (+0x26 + 12c + v),
    /// already looked up by the caller: `codes[c][v]`.
    pub codes: [[Code; 12]; 16],
    /// monstats2 `compositeDeath`.
    pub composite_death: bool,
    /// The level value `0x006427F0` of the unit's room is 1 (override
    /// tables, open question 5).
    pub override_level: bool,
}

/// What §5.1 r3 reads for a player.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlayerLook {
    /// Valid body armor (location 3): `torso`, `legs`, `rArm`, `lArm`,
    /// `rspad`, `lspad` bytes (+0x116 … +0x11B).
    pub body_armor: Option<[u8; 6]>,
    /// Gfx code (`alternategfx`, else `code`) of the valid equipped item
    /// for each component (`0x0063C050`: for 5 the primary weapon, for 6
    /// the other hand item).
    pub item_gfx: [Option<Code>; 16],
    /// State 101 `holyshield`.
    pub holy_shield: bool,
    /// An item in the shield hand (`0x0063C8F0`).
    pub shield_hand_item: bool,
    /// The inventory is the linked unit's (§1 substitution, open question
    /// 2); the caller cannot tell yet, so `true` is refused.
    pub linked_inventory: bool,
}

/// Index into `torso` … `lspad` of the armor-class components (`0x0064F420`).
fn armor_slot(c: u8) -> Option<usize> {
    use component::*;
    match c {
        TR => Some(0),
        LG => Some(1),
        RA => Some(2),
        LA => Some(3),
        S1 => Some(4),
        S2 => Some(5),
        _ => None,
    }
}

/// The armor class of component `c` (§5.1 r1–r3), or `None` when the
/// request fails (the slot draws nothing). `mode` is the unit's mode and
/// `weapon_class` the request's (§2.1).
pub fn armor_class(
    source: ArmorSource<'_>,
    c: u8,
    mode: u8,
    weapon_class: Code,
) -> Result<Option<Code>, UnitCompositeError> {
    use component::*;
    let c16 = usize::from(c & 15);
    match source {
        ArmorSource::Object => Ok(Some(LIT)),
        ArmorSource::Monster(m) => {
            if m.override_level {
                // TODO(spec: render/unit-composite.md OQ5): tables
                // `0x007489A8` / `0x00748A18` and their conditions.
                return Err(unresolved("monster armor-class override", "OQ5"));
            }
            if CompositeKind::Monster.is_death_mode(mode) && !m.composite_death {
                return Ok(Some(LIT));
            }
            let v = m.choices[c16];
            if v < m.counts[c16] && usize::from(v) < 12 {
                Ok(Some(m.codes[c16][usize::from(v)]))
            } else {
                Ok(Some(LIT))
            }
        }
        ArmorSource::Player(None) => Ok(None),
        ArmorSource::Player(Some(p)) => {
            if let Some(i) = armor_slot(c) {
                if CompositeKind::Player.is_death_mode(mode) {
                    return Ok(Some(LIT));
                }
                let index = p.body_armor.map_or(0, |b| b[i]);
                // An index above 2 reads past `armtype` (Edge cases); no
                // 1.14d row has one: the request fails (armor class 0).
                return Ok(ARMTYPE.get(usize::from(index)).copied());
            }
            if p.linked_inventory {
                // TODO(spec: render/unit-composite.md OQ2).
                return Err(unresolved("linked-unit inventory", "OQ2"));
            }
            let gfx = |c: u8| p.item_gfx[usize::from(c)].unwrap_or(LIT);
            Ok(Some(match c {
                RH | LH if part(&weapon_class) == b"xbw" => gfx(RH),
                RH if part(&weapon_class) == b"bow" => LIT,
                SH if p.holy_shield && p.shield_hand_item => HSH,
                _ => gfx(c),
            }))
        }
    }
}

impl ComponentCodes {
    /// The request of §5.1, or `None` when it fails (empty unit token,
    /// component token or weapon class; armor class 0). An empty mode
    /// token becomes `xxx `.
    pub fn new(
        kind: CompositeKind,
        unit_token: Code,
        component_token: Code,
        armor_class: Option<Code>,
        mode_token: Code,
        weapon_class: Code,
    ) -> Option<ComponentCodes> {
        let armor_class = armor_class.filter(|a| *a != EMPTY)?;
        if part(&unit_token).is_empty()
            || part(&component_token).is_empty()
            || part(&weapon_class).is_empty()
        {
            return None;
        }
        let mode_token = if part(&mode_token).is_empty() {
            XXX
        } else {
            mode_token
        };
        Some(ComponentCodes {
            kind,
            unit_token,
            component_token,
            armor_class,
            mode_token,
            weapon_class,
        })
    }

    /// File name (`0x005FE2B0`, §6 r1): the five parts.
    pub fn name(&self) -> String {
        [
            &self.unit_token,
            &self.component_token,
            &self.armor_class,
            &self.mode_token,
            &self.weapon_class,
        ]
        .iter()
        .map(|c| text(part(c)))
        .collect()
    }

    /// Path (`0x005FE610`, §6 r1): `<root>\<T>\<C>\<name>.<ext>`.
    pub fn file(&self, format: FileFormat) -> String {
        format!(
            "{}\\{}\\{}\\{}.{}",
            self.kind.root(),
            text(part(&self.unit_token)),
            text(part(&self.component_token)),
            self.name(),
            format.extension()
        )
    }

    /// [`ComponentCodes::file`] as an asset path.
    pub fn path(&self, format: FileFormat) -> Result<CanonicalPath, UnitCompositeError> {
        canonical(self.file(format))
    }
}

/// Component file format (§6 r2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileFormat {
    Dcc,
    Dc6,
}

impl FileFormat {
    pub fn extension(self) -> &'static str {
        match self {
            FileFormat::Dcc => "dcc",
            FileFormat::Dc6 => "dc6",
        }
    }
}

/// Monsters always drawn from DC6 (§6 r2; `monstats` row indices).
pub const DC6_MONSTERS: [u32; 5] = [242, 251, 367, 521, 704];
/// Monsters drawn from DC6 in mode 0 (DT) only.
pub const DC6_MONSTERS_DEATH: [u32; 8] = [243, 284, 333, 544, 559, 570, 705, 709];
/// Objects always drawn from DC6 (`objects` row indices).
pub const DC6_OBJECTS: [u32; 2] = [342, 563];
/// The one file name drawn from DC6 by name.
pub const DC6_NAME: &str = "OYTRlitTNhth";

/// §6 r2 with `CompressedData` = 1 (the only supported value, open
/// question 12). `class` is the `monstats` / `objects` row index.
pub fn file_format(codes: &ComponentCodes, class: u32, mode: u8) -> FileFormat {
    let dc6 = match codes.kind {
        CompositeKind::Monster => {
            DC6_MONSTERS.contains(&class) || (mode == 0 && DC6_MONSTERS_DEATH.contains(&class))
        }
        CompositeKind::Object => DC6_OBJECTS.contains(&class),
        CompositeKind::Player => false,
    };
    // TODO(spec: render/unit-composite.md §6 r2): case of the name
    // comparison is not stated; compared byte for byte.
    if dc6 || codes.name() == DC6_NAME {
        FileFormat::Dc6
    } else {
        FileFormat::Dcc
    }
}

// ---------------------------------------------------------------------------
// §7 colormap source.

/// The unit map `U` (§7 r1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnitMap {
    None,
    /// Shift table row (unit +0x6C, `0x004FB0C0`).
    ShiftRow(u8),
    /// The monster's palette shift (`0x00477530`, `palshift.dat`).
    MonsterPalShift,
}

/// §7 r1: palette index ≠ 0 → shift row; else monsters' palette shift;
/// else none.
pub fn unit_map(palette_index: u8, kind: CompositeKind) -> UnitMap {
    if palette_index != 0 {
        UnitMap::ShiftRow(palette_index)
    } else if kind == CompositeKind::Monster {
        UnitMap::MonsterPalShift
    } else {
        UnitMap::None
    }
}

/// The colormap source of one slot (§7). The maps themselves are
/// `render/shading.md`'s.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColormapSource {
    None,
    Unit(UnitMap),
    /// The item colour of the equipped item of `component`
    /// (`0x0062C100`), `fallback` when that gives none.
    Item {
        component: u8,
        fallback: UnitMap,
    },
    /// S8 local blood (`0x00477680`, open question 6).
    Blood,
}

/// One equipped item as §7 r2 reads it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ItemLook {
    pub flags: u32,
    /// Gfx code (§5.1 r3).
    pub gfx: Code,
}

/// What §7 reads beyond the component.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ColormapInputs<'a> {
    pub kind: CompositeKind,
    pub unit: UnitMap,
    /// Unit +0x6C.
    pub palette_index: u8,
    /// The item-map condition of §7 r2 (`0x0063A790(unit)` = 0, or the
    /// local player with `0x00477750` ≠ 0), evaluated by the caller.
    pub item_maps: bool,
    /// Equipped items by component (`0x0063C050(c)`), players only.
    pub items: &'a [Option<ItemLook>; 16],
    /// monstats2 `localBlood` ≠ 0 and `0x0044DC60` ≠ 0 (open question 6).
    pub local_blood: bool,
}

/// §7: the colormap source of component `c`.
pub fn colormap_source(c: u8, inputs: &ColormapInputs<'_>) -> ColormapSource {
    use component::*;
    match c {
        S8 => {
            if inputs.kind == CompositeKind::Monster && inputs.local_blood {
                ColormapSource::Blood
            } else {
                ColormapSource::None
            }
        }
        S7 => ColormapSource::None,
        _ => {
            let player = inputs.kind == CompositeKind::Player;
            if !player || !inputs.item_maps || inputs.palette_index == 0x6C {
                return ColormapSource::Unit(inputs.unit);
            }
            let own = inputs.items.get(usize::from(c)).copied().flatten();
            let (component, item) = match own {
                Some(i) => (c, Some(i)),
                None if matches!(c, LG | RA | LA | S1 | S2) => (TR, inputs.items[usize::from(TR)]),
                None => (c, None),
            };
            match item {
                Some(i) if i.flags & (0x100 | 0x4000) == 0 => {
                    if part(&i.gfx) == b"lit" {
                        ColormapSource::None
                    } else {
                        ColormapSource::Item {
                            component,
                            fallback: inputs.unit,
                        }
                    }
                }
                _ => ColormapSource::Unit(inputs.unit),
            }
        }
    }
}

// ---------------------------------------------------------------------------
// §8 extra offsets.

/// Client motion record flags (§8).
pub mod motion {
    pub const DONE: i32 = 1;
    pub const TIMED: i32 = 2;
    pub const BOUNCE: i32 = 4;
    pub const FOLLOW: i32 = 0x10;
    pub const FROM_BELOW: i32 = 0x20;
}

/// The 0x4C-byte client motion record (§8). Positions are 16.16 subtile
/// (`>> 11` is 1/32 subtile), limits 1/32 subtile, offsets pixels.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MotionRecord {
    pub flags: i32,
    pub pos: [i32; 3],
    pub vel: [i32; 3],
    pub acc: [i32; 3],
    pub limit: [i32; 3],
    /// `ox`, `oy`, `oz` (read by the draw).
    pub offset: [i32; 3],
    pub bounces_left: i32,
    /// Percent.
    pub bounce_factor: i32,
    pub ticks_left: i32,
}

impl MotionRecord {
    /// One update (`0x004DA350`, §8 r1–r6), once per client unit update;
    /// nothing when done. The follow branch (flag 0x10) is open question 7.
    pub fn update(&mut self) -> Result<(), UnitCompositeError> {
        use motion::*;
        if self.flags & DONE != 0 {
            return Ok(());
        }
        let [x, y, z] = &mut self.pos;
        let [vx, vy, vz] = &mut self.vel;
        let [ax, ay, az] = self.acc;
        // r1, in this order.
        *x = x.wrapping_add(*vx);
        *y = y.wrapping_add(*vy);
        *vx = vx.wrapping_add(ax);
        *z = z.wrapping_add(*vz);
        *vy = vy.wrapping_add(ay);
        *vz = vz.wrapping_add(az);
        // r2.
        if self.flags & TIMED != 0 {
            if self.ticks_left == 0 {
                self.flags |= DONE;
                self.pos[0] = 0;
                self.pos[1] = 0;
            } else {
                self.ticks_left -= 1;
            }
        }
        let [lx, ly, lz] = self.limit;
        if self.flags & BOUNCE != 0 {
            // r3.
            if self.pos[2] >> 11 <= lz {
                self.vel[2] = self
                    .bounce_factor
                    .wrapping_mul(self.vel[2])
                    .wrapping_div(100)
                    .wrapping_neg();
                self.pos[2] = lz;
                if self.bounces_left != 0 {
                    self.bounces_left -= 1;
                } else {
                    self.flags |= DONE;
                }
            }
        } else if self.flags & FOLLOW != 0 {
            // r4. TODO(spec: render/unit-composite.md OQ7): `0x004706E0`.
            return Err(unresolved("motion record follow branch", "OQ7"));
        } else {
            // r5.
            let [a, b, c] = self.pos.map(|p| p >> 11);
            let stop = if self.flags & FROM_BELOW == 0 {
                a <= lx && b <= ly && c <= lz
            } else {
                a >= lx && b >= ly && c >= lz
            };
            if stop {
                self.pos = [lx << 11, ly << 11, lz << 11];
                self.flags |= DONE;
            }
        }
        // r6 (flag 0x10 returned above).
        let a = self.pos[0] >> 11;
        let b = self.pos[1] >> 11;
        self.offset = [
            a.wrapping_sub(b) >> 1,
            a.wrapping_add(b) >> 2,
            (self.pos[2] >> 11).wrapping_neg(),
        ];
        Ok(())
    }

    /// The draw's addition `(ox, oy + oz)` (§8).
    pub fn draw_offset(&self) -> (i32, i32) {
        let [ox, oy, oz] = self.offset;
        (ox, oy.wrapping_add(oz))
    }
}

/// The table offsets of §8 added after the motion record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableOffset {
    None,
    /// `objects` Xoffset, Yoffset, Draw.
    Object {
        x: i32,
        y: i32,
        draw: bool,
    },
    /// `missiles` xoffset, yoffset, zoffset; `None` = no missiles row.
    Missile(Option<(i16, i16, i16)>),
}

/// `ViewSource::unit_offset` (§8, §10): `(ox, oy + oz)` of the record
/// (0 without one) plus the object / missile offsets; `None` when the unit
/// is not drawn (object Draw = 0, missile without a row).
pub fn unit_offset(record: Option<&MotionRecord>, table: TableOffset) -> Option<(i32, i32)> {
    let (x, y) = record.map_or((0, 0), MotionRecord::draw_offset);
    match table {
        TableOffset::None => Some((x, y)),
        TableOffset::Object { draw: false, .. } => None,
        TableOffset::Object {
            x: ox,
            y: oy,
            draw: true,
        } => Some((x.wrapping_add(ox), y.wrapping_add(oy))),
        TableOffset::Missile(None) => None,
        TableOffset::Missile(Some((mx, my, mz))) => Some((
            x.wrapping_add(i32::from(mx)),
            y.wrapping_add(i32::from(my)).wrapping_add(i32::from(mz)),
        )),
    }
}

// ---------------------------------------------------------------------------
// §9 single-cel units.

/// `DATA\GLOBAL\MISSILES\<CelFile>.dcc` (§9, `CompressedData` = 1).
pub fn missile_file(cel_file: &str) -> String {
    format!("DATA\\GLOBAL\\MISSILES\\{cel_file}.dcc")
}

/// Which graphic an item shows by mode (`0x004DAA70`, §9).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemGraphic {
    Inventory,
    Ground,
}

pub fn item_graphic(mode: u8) -> Option<ItemGraphic> {
    match mode {
        0 | 1 | 2 | 4 | 6 => Some(ItemGraphic::Inventory),
        3 | 5 => Some(ItemGraphic::Ground),
        _ => None,
    }
}

/// A gold pile's direction by the amount of stat 14 (§9).
pub fn gold_direction(amount: u32) -> u8 {
    match amount {
        0..=99 => 0,
        100..=499 => 1,
        500..=4999 => 2,
        _ => 3,
    }
}

/// Quality values of §9.
pub const QUALITY_SET: u8 = 5;
pub const QUALITY_UNIQUE: u8 = 7;

/// A ground item's flippy file (`0x004DABC0`, §9): the unique / set
/// flippy file when the quality is 7 / 5 and that name is not empty, else
/// the item's own; path `DATA\GLOBAL\items\<name>.dc6`.
pub fn flippy_file(own: &str, quality: u8, unique: Option<&str>, set: Option<&str>) -> String {
    let special = match quality {
        QUALITY_UNIQUE => unique,
        QUALITY_SET => set,
        _ => None,
    }
    .filter(|n| !n.is_empty());
    format!("DATA\\GLOBAL\\items\\{}.dc6", special.unwrap_or(own))
}

#[cfg(test)]
#[path = "unit_composite_tests.rs"]
mod tests;
