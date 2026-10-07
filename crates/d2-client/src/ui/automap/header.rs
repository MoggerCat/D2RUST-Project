// Spec: specs/ui/automap.md (§13)
//! The header text of the automap pass (§13): game name and password,
//! level name, version, difficulty, game type text and the expansion line.

use super::draw::{AutomapDraw, Label, TextAlign};
use super::AutomapError;

/// String ids (§13; labels pending, open question 3).
pub const GAME_NAME: u16 = 4181;
pub const PASSWORD: u16 = 4182;
pub const DIFFICULTY: u16 = 4183;
pub const NIGHTMARE: u16 = 5154;
pub const HELL: u16 = 5155;
pub const EXPANSION: u16 = 22730;
/// The difficulty line's limit (fatal 0xB2A above it).
pub const DIFFICULTY_MAX: usize = 299;
/// Line y: starts at 24, steps 16 (`[0x007A51BC]`).
pub const FIRST_Y: i32 = 24;
pub const STEP_Y: i32 = 16;

/// What the header reads.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HeaderFacts {
    /// `0x0044B7C0`, `0x0044B7F0` (empty: absent).
    pub game_name: Vec<u16>,
    pub password: Vec<u16>,
    /// `0x00453E70` of the player's room's level.
    pub level_name: Vec<u16>,
    /// `0x0044DCD0`: 0 Normal, 1 Nightmare, 2 Hell.
    pub difficulty: u8,
    /// `0x0044DB30`.
    pub game_type: u32,
    /// The `0x0040DF60` text for game types 6 and 8.
    pub game_type_text: Vec<u16>,
    pub expansion: bool,
}

fn utf16(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

/// §13: the header lines, each right-aligned at x = W − 16 − width
/// (`Right` alignment on `W − 16`), font 1, colour 4. `strings` looks a
/// string id up; a missing string is an error.
pub fn header(
    f: &HeaderFacts,
    width: i32,
    strings: &dyn Fn(u16) -> Option<Vec<u16>>,
    out: &mut Vec<AutomapDraw>,
) -> Result<(), AutomapError> {
    let get = |id: u16| {
        strings(id)
            .ok_or_else(|| AutomapError::Unresolved(format!("§13: string {id} not in the tables")))
    };
    let join = |id: u16, tail: &[u16]| -> Result<Vec<u16>, AutomapError> {
        let mut t = get(id)?;
        t.extend_from_slice(tail);
        Ok(t)
    };
    let mut lines = Vec::new();
    // r1.
    if !f.game_name.is_empty() {
        lines.push(join(GAME_NAME, &f.game_name)?);
    }
    if !f.password.is_empty() {
        lines.push(join(PASSWORD, &f.password)?);
    }
    // r2.
    if !f.level_name.is_empty() {
        lines.push(f.level_name.clone());
    }
    // r3: `v %d.%d%c` with 1, 14, `d`.
    lines.push(utf16(&format!("v {}.{}{}", 1, 14, 'd')));
    // r4.
    if matches!(f.difficulty, 1 | 2) {
        let d = get(if f.difficulty == 1 { NIGHTMARE } else { HELL })?;
        let t = join(DIFFICULTY, &d)?;
        if t.len() > DIFFICULTY_MAX {
            return Err(AutomapError::fatal(
                0xB2A,
                "§13 r4",
                format!("difficulty line of {} units", t.len()),
            ));
        }
        lines.push(t);
    }
    // r5.
    if matches!(f.game_type, 6 | 8) && !f.game_type_text.is_empty() {
        lines.push(f.game_type_text.clone());
    }
    // r6.
    if f.expansion {
        lines.push(get(EXPANSION)?);
    }
    for (i, t) in lines.into_iter().enumerate() {
        out.push(AutomapDraw::Text {
            text: Label::Text(t),
            font: 1,
            color: 4,
            x: width - 16,
            y: FIRST_Y + STEP_Y * i as i32,
            align: TextAlign::Right,
        });
    }
    Ok(())
}
