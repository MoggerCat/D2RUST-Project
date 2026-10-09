// Spec: specs/ui/automap.md (§11, §12)
//! Unit markers (`0x0045AC90` → `0x0045A860`) and party roster markers
//! (`0x0045AB60`): the colour table, the cross shape and the names.

use super::draw::{AutomapDraw, Label, TextAlign};
use super::view::Bounds;
use crate::rules::camera::{static_to_client, ClientPos};

/// The marker colours (§11 r3), made at act load (`0x0045A620`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum MarkerColor {
    B0,
    B1,
    B2,
    B3,
    B4,
    B5,
    B6,
    B8,
    /// Palette index 0 (object 267).
    Index0,
}

impl MarkerColor {
    /// The (r, g, b) the act load matches (§11 r3); `None` for index 0.
    pub fn rgb(self) -> Option<(u8, u8, u8)> {
        Some(match self {
            MarkerColor::B0 => (0, 0, 255),
            MarkerColor::B1 => (255, 0, 0),
            MarkerColor::B2 => (255, 0, 255),
            MarkerColor::B3 => (0, 255, 0),
            MarkerColor::B4 => (0x44, 0x70, 0x74),
            MarkerColor::B5 => (0x48, 0xA0, 0x34),
            MarkerColor::B6 => (0xF4, 0xF4, 0xF4),
            MarkerColor::B8 => (0xF4, 0xF4, 0),
            MarkerColor::Index0 => return None,
        })
    }
}

/// The palette indices of the colours (§14 r2, `0x0045A620`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct MarkerPalette {
    /// Indexed by [`MarkerColor`] order.
    pub index: [u8; 9],
}

impl MarkerPalette {
    /// `nearest` is the act palette's `0x004FB180` match.
    pub fn new(nearest: impl Fn(u8, u8, u8) -> u8) -> Self {
        const ALL: [MarkerColor; 9] = [
            MarkerColor::B0,
            MarkerColor::B1,
            MarkerColor::B2,
            MarkerColor::B3,
            MarkerColor::B4,
            MarkerColor::B5,
            MarkerColor::B6,
            MarkerColor::B8,
            MarkerColor::Index0,
        ];
        MarkerPalette {
            index: ALL.map(|c| c.rgb().map_or(0, |(r, g, b)| nearest(r, g, b))),
        }
    }

    pub fn get(&self, c: MarkerColor) -> u8 {
        self.index[c as usize]
    }
}

/// What a marker reads of a unit (§11).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MarkerSubject {
    Player {
        local: bool,
        mode: u32,
        /// Party id (−1 none).
        party: i16,
        /// Inventory owner (`0x0063D450`) is the unit itself.
        own_inventory: bool,
        /// State 7 (`0x00639DF0`).
        state_7: bool,
        name: Vec<u16>,
    },
    Monster {
        class: u32,
        mode: u32,
        /// Flags +0xC4.
        flags: u32,
        /// `monstats` `interact`.
        interact: bool,
        /// Disguised as a player (state 0x25 and an owner): whether the
        /// owner is in the local player's party.
        disguised: Option<bool>,
        /// `0x00478D90` relation code (§11 r3: roster relation; in
        /// single player the local player's own pets and hireling give 1).
        relation: u8,
        /// The monster's own name (`0x00464A60`, the interact name).
        name: Vec<u16>,
        /// The owner's name of a disguised monster (§11 r6).
        owner_name: Vec<u16>,
    },
    Object {
        class: u32,
        /// Target level of a portal (object 60).
        target_level: Option<u32>,
    },
    Other,
}

/// One unit of a near room's list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkerUnit {
    pub subject: MarkerSubject,
    /// Client position (§4 r4).
    pub pos: ClientPos,
}

/// The pass facts the markers read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MarkerCtx {
    /// The local player's party id (−1 none).
    pub local_party: i16,
    /// `AutoMap Party`, `AutoMap Party Names`.
    pub party: bool,
    pub names: bool,
    /// `0x00464820` ≠ 0 for the player ([`unit_dead`], §11 r1).
    pub player_gate: bool,
    pub mini: bool,
    pub div: i32,
    /// (Ax, Ay), §9 r3.
    pub a: (i32, i32),
    /// The marker rectangle, §9 r1.
    pub rect: Bounds,
    pub palette: MarkerPalette,
}

/// Monster mode 12 (dead) and flag bit 21 skip the colour (§11 r3).
const MONSTER_DEAD: u32 = 12;
const MONSTER_FLAG_21: u32 = 1 << 21;
/// Player mode 17 (dead).
const PLAYER_DEAD: u32 = 17;
/// Portal targets without a marker (§11 r3).
const UNMARKED_PORTALS: [u32; 6] = [111, 112, 117, 125, 126, 127];
/// String 0xCF3, object 267's label (§11 r6).
pub const OBJECT_267_STRING: u16 = 0xCF3;

/// §11 r3 (`0x00459BC0`): the colour of a unit, `None` for no marker.
pub fn color(s: &MarkerSubject, ctx: &MarkerCtx) -> Option<MarkerColor> {
    match *s {
        MarkerSubject::Player {
            local,
            mode,
            party,
            own_inventory,
            ..
        } => {
            if mode != PLAYER_DEAD {
                Some(if local {
                    MarkerColor::B0
                } else if party != -1 && party == ctx.local_party {
                    MarkerColor::B3
                } else {
                    MarkerColor::B1
                })
            } else {
                own_inventory.then_some(MarkerColor::B2)
            }
        }
        MarkerSubject::Monster {
            class,
            mode,
            flags,
            interact,
            disguised,
            relation,
            ..
        } => {
            if mode == MONSTER_DEAD || flags & MONSTER_FLAG_21 != 0 {
                return None;
            }
            if interact {
                return (!(537..=539).contains(&class)).then_some(MarkerColor::B6);
            }
            if let Some(same) = disguised {
                return Some(if same {
                    MarkerColor::B4
                } else {
                    MarkerColor::B1
                });
            }
            match relation {
                1 => Some(MarkerColor::B4),
                2 if ctx.party => Some(MarkerColor::B5),
                _ => None,
            }
        }
        MarkerSubject::Object {
            class,
            target_level,
        } => match class {
            59 => Some(MarkerColor::B8),
            60 if !target_level.is_some_and(|t| UNMARKED_PORTALS.contains(&t)) => {
                Some(MarkerColor::B8)
            }
            267 => Some(MarkerColor::Index0),
            _ => None,
        },
        MarkerSubject::Other => None,
    }
}

/// The 13 points of table `0x006D6638` (§11 r4).
pub const SHAPE: [(i32, i32); 13] = [
    (0, -1),
    (2, -2),
    (4, -1),
    (2, 0),
    (4, 1),
    (2, 2),
    (0, 1),
    (-2, 2),
    (-4, 1),
    (-2, 0),
    (-4, -1),
    (-2, -2),
    (0, -1),
];

/// §11 r4 (`0x0045A7F0`): 12 lines joining the 13 points ×2 around
/// (X, Y); mini: around (X − 1, Y + 5).
pub fn cross(x: i32, y: i32, mini: bool, color: u8, out: &mut Vec<AutomapDraw>) {
    let (x, y) = if mini { (x - 1, y + 5) } else { (x, y) };
    for p in SHAPE.windows(2) {
        out.push(AutomapDraw::Line {
            from: (x + 2 * p[0].0, y + 2 * p[0].1),
            to: (x + 2 * p[1].0, y + 2 * p[1].1),
            color,
        });
    }
}

/// §11 r7 (`0x0045A760`): font 6, colour (≥ 13 → 0), centred on x, top
/// at y − 10; empty → nothing.
pub fn name(text: &[u16], x: i32, y: i32, color: u16, out: &mut Vec<AutomapDraw>) {
    if text.is_empty() {
        return;
    }
    out.push(AutomapDraw::Text {
        text: Label::Text(text.to_vec()),
        font: 6,
        color: if color >= 13 { 0 } else { color },
        x,
        y: y - 10,
        align: TextAlign::Centre,
    });
}

/// The screen position of a marker (§11 r2, §12 r2).
fn screen(p: ClientPos, ctx: &MarkerCtx) -> (i32, i32) {
    (p.x / ctx.div - ctx.a.0 + 8, p.y / ctx.div - ctx.a.1 - 8)
}

/// §11 r1–r7: the markers of the near rooms' units, in list order.
pub fn unit_markers(units: &[MarkerUnit], ctx: &MarkerCtx, out: &mut Vec<AutomapDraw>) {
    for u in units {
        // r1.
        if let MarkerSubject::Player { state_7, .. } = u.subject {
            if ctx.player_gate && !state_7 {
                continue;
            }
        }
        let Some(c) = color(&u.subject, ctx) else {
            continue;
        };
        // r2.
        let (x, y) = screen(u.pos, ctx);
        if !ctx.rect.contains(x, y) {
            continue;
        }
        let idx = ctx.palette.get(c);
        match &u.subject {
            // r5.
            MarkerSubject::Player { local, name: n, .. } => {
                if !(c == MarkerColor::B3 && !ctx.party) {
                    cross(x, y, ctx.mini, idx, out);
                }
                // §11 r5: the colour is an immediate (2 for a party member
                // marker, else 1), not the marker's palette byte.
                if !local && ctx.party && ctx.names {
                    name(n, x, y, if c == MarkerColor::B3 { 2 } else { 1 }, out);
                }
            }
            // r6.
            MarkerSubject::Monster {
                interact,
                disguised,
                name: n,
                owner_name,
                ..
            } => {
                cross(x, y, ctx.mini, idx, out);
                if *interact && ctx.names {
                    // Font 6, colour 4, centred at (X, Y + 8 − 18): the
                    // r7 placement with y = Y.
                    name(n, x, y, 4, out);
                }
                // Then, independently: a disguised monster whose owner is
                // not in the local party gets the owner's name, colour 1.
                if *disguised == Some(false) && ctx.party && ctx.names {
                    name(owner_name, x, y, 1, out);
                }
            }
            MarkerSubject::Object { class: 267, .. } if ctx.names => {
                // Centred at (X, Y + 8 − 18) instead of a cross.
                out.push(AutomapDraw::Text {
                    text: Label::String(OBJECT_267_STRING),
                    font: 6,
                    color: 4,
                    x,
                    y: y - 10,
                    align: TextAlign::Centre,
                });
            }
            _ => cross(x, y, ctx.mini, idx, out),
        }
    }
}

/// One player roster entry (§12, `client/msg-units.md` §8).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RosterEntry {
    /// +0x22 (−1 none).
    pub party: i16,
    /// The act of the entry's level (+0x24).
    pub act: u8,
    /// +0x28, +0x2C: subtile position from S→C 0x90 (§12 r1).
    pub x: i32,
    pub y: i32,
    /// `0x00463990` found a client unit.
    pub has_unit: bool,
    pub name: Vec<u16>,
}

/// §12 r2: crosses (and names) of party members without a client unit.
pub fn roster_markers(
    roster: &[RosterEntry],
    local_act: u8,
    ctx: &MarkerCtx,
    out: &mut Vec<AutomapDraw>,
) {
    let p = ctx.local_party;
    // `0x00479BC0`: a party id shared by ≥ 2 roster entries.
    let has_party = p != -1 && roster.iter().filter(|e| e.party == p).count() >= 2;
    if !has_party || !ctx.party {
        return;
    }
    let idx = ctx.palette.get(MarkerColor::B3);
    for e in roster {
        if e.party != p || e.has_unit || e.act != local_act {
            continue;
        }
        let (x, y) = screen(static_to_client(e.x, e.y), ctx);
        if !ctx.rect.contains(x, y) {
            continue;
        }
        cross(x, y, ctx.mini, idx, out);
        if ctx.names {
            name(&e.name, x, y, u16::from(idx), out);
        }
    }
}

/// Unit types `0x00464820` distinguishes (§11 r1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeadKind {
    Player,
    Monster,
    Other,
}

/// `0x00464820` (§11 r1): "dead" = flag +0xC4 bit 16 (0x10000) set, or
/// a player in mode 0 / 17, or a monster in mode 0 / 12; other types:
/// only the flag. Gives [`MarkerCtx::player_gate`] for a player unit.
pub fn unit_dead(kind: DeadKind, mode: u32, flags: u32) -> bool {
    flags & 0x1_0000 != 0
        || match kind {
            DeadKind::Player => matches!(mode, 0 | PLAYER_DEAD),
            DeadKind::Monster => matches!(mode, 0 | MONSTER_DEAD),
            DeadKind::Other => false,
        }
}
