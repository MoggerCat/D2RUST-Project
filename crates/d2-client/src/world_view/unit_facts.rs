// Spec: specs/render/draw-order.md (§3 r4, §5), specs/render/draw-order-2.md (§15), specs/client/msg-units.md
//! The room-unit facts the draw order reads (`UnitFacts`) from the client
//! model: unit flags (+0xC4) and flag-ex (+0xC8) as the message rules
//! leave them, the states 7 `playerbody`, 143 `attached` and 146 `invis`,
//! the per-class table columns (monstats2 `unflatDead`, objects
//! `DrawUnder`) and the level gate of the sight test (`LOSDraw`).
//!
//! PROVISIONAL (REC-273): the unit flag bits no model rule writes (flat
//! 0x100000, the dead bit 0x10000 of a missile) read 0, and the line test
//! of the sight test needs the client DRLG's collision grid, which the
//! feed does not hold: in a `LOSDraw` level `sight_hidden` stays `None`
//! (the draw order then refuses the frame, as the spec's gate requires),
//! elsewhere every unit passes (§15 r1).

use d2_data::tables::{decode_all, Leveldefs, Monstats, Monstats2, Objects, Record};

use crate::bridge::world::{ClientUnit, ClientWorld, LevelRow, MONSTER, OBJECT};
use crate::rules::draw_order::UnitFacts;

use super::ViewError;

/// Unit flag +0xC4 bits the model holds (`model.md` §8 r7, §14 r3, §5 r5;
/// `msg-ui.md` §1 r4).
pub const FLAG_2: u32 = 0x2;
pub const FLAG_4: u32 = 0x4;
pub const FLAG_200: u32 = 0x200;
pub const FLAG_ROOM_FREED: u32 = 0x80_0000;

/// States the draw order reads (`draw-order.md` §5).
pub const STATE_PLAYERBODY: u8 = 7;
pub const STATE_ATTACHED: u8 = 143;
pub const STATE_INVIS: u8 = 146;

/// The table columns the facts read, by class.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UnitFactTables {
    /// `monstats2` `unflatDead` by `monstats` row (through `MonStatsEx`);
    /// `None`: no `monstats2` row.
    pub unflat_dead: Vec<Option<bool>>,
    /// `objects` `DrawUnder` by class.
    pub draw_under: Vec<u8>,
    /// leveldefs `LOSDraw` by level id.
    pub los_draw: Vec<bool>,
}

impl UnitFactTables {
    /// From the typed rows.
    pub fn from_rows(
        monstats: &[Monstats],
        monstats2: &[Monstats2],
        objects: &[Objects],
        defs: &[Leveldefs],
    ) -> Self {
        Self {
            unflat_dead: monstats
                .iter()
                .map(|m| {
                    monstats2
                        .get(usize::from(m.monstatsex))
                        .map(|r| r.unflatdead)
                })
                .collect(),
            draw_under: objects.iter().map(|o| o.drawunder).collect(),
            los_draw: defs.iter().map(|d| d.losdraw != 0).collect(),
        }
    }
}

/// The columns from the user's tables.
pub fn load(archives: &dyn d2_data::bin::TableFiles) -> Result<UnitFactTables, String> {
    let set = d2_data::bin::load_from(archives, "eng").map_err(|e| e.to_string())?;
    fn all<R: Record>(set: &d2_data::bin::BinSet, name: &str) -> Result<Vec<R>, String> {
        let t = set
            .table(name)
            .ok_or_else(|| format!("{name} not loaded"))?;
        decode_all(t).map_err(|e| e.to_string())
    }
    Ok(UnitFactTables::from_rows(
        &all::<Monstats>(&set, "monstats")?,
        &all::<Monstats2>(&set, "monstats2")?,
        &all::<Objects>(&set, "objects")?,
        &all::<Leveldefs>(&set, "leveldefs")?,
    ))
}

fn unresolved(what: &'static str, message: String) -> ViewError {
    ViewError::Unresolved {
        what,
        spec: "render/draw-order.md",
        message,
    }
}

/// The unit flag word +0xC4 as far as the model writes it.
pub fn flags_of(unit: &ClientUnit) -> u32 {
    let mut f = 0;
    if unit.flag_2 == Some(true) && !unit.quest_untargetable {
        f |= FLAG_2;
    }
    if unit.flag_4 {
        f |= FLAG_4;
    }
    if unit.flag_200 {
        f |= FLAG_200;
    }
    if unit.room_freed {
        f |= FLAG_ROOM_FREED;
    }
    f
}

/// Writes the model's facts of `unit` into `f` (flags, flag-ex, the three
/// states, `unflatDead`, `DrawUnder`); `f.flag_ex` keeps the bits it
/// already has (the draw's own, 0x80).
pub fn fill_model(
    f: &mut UnitFacts,
    unit: &ClientUnit,
    t: &UnitFactTables,
) -> Result<(), ViewError> {
    f.flags |= flags_of(unit);
    f.flag_ex |= unit.flag_ex;
    f.playerbody = unit.states.contains(&STATE_PLAYERBODY);
    f.attached = unit.states.contains(&STATE_ATTACHED);
    f.invis = unit.states.contains(&STATE_INVIS);
    let class = unit.class as usize;
    match unit.key.unit_type {
        MONSTER => {
            f.unflat_dead = t.unflat_dead.get(class).copied().flatten().ok_or_else(|| {
                unresolved(
                    "monstats2 unflatDead",
                    format!("monster class {class} has no monstats2 row"),
                )
            })?;
        }
        OBJECT => {
            f.draw_under = t.draw_under.get(class).copied().ok_or_else(|| {
                unresolved(
                    "objects DrawUnder",
                    format!("object class {class} past the objects rows"),
                )
            })?;
        }
        _ => {}
    }
    Ok(())
}

/// The sight test's answer (`draw-order-2.md` §15 r1): `Some(false)` when
/// the local player's level has `LOSDraw` 0, else `None` (the line test
/// needs the collision grid; PROVISIONAL REC-273).
pub fn sight_gate(world: &ClientWorld, t: &UnitFactTables) -> Option<bool> {
    let level = world.player_level()?;
    match t.los_draw.get(usize::from(level)) {
        Some(false) | None => Some(false),
        Some(true) => None,
    }
}

/// The facts of a room unit from the model alone.
pub fn model_facts(
    world: &ClientWorld,
    unit: &ClientUnit,
    t: &UnitFactTables,
    _levels: Option<&[LevelRow]>,
) -> Result<UnitFacts, ViewError> {
    let mut f = UnitFacts {
        unit_type: unit.key.unit_type,
        mode: unit.mode,
        local: world.local_player == Some(unit.key),
        sight_hidden: sight_gate(world, t),
        ..UnitFacts::default()
    };
    fill_model(&mut f, unit, t)?;
    Ok(f)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bridge::world::{UnitKey, PLAYER};
    use crate::rules::draw_order::{is_flat, UNIT_EX_VISIBLE};

    fn tables() -> UnitFactTables {
        UnitFactTables {
            unflat_dead: vec![Some(false), Some(true), None],
            draw_under: vec![0, 2, 1],
            los_draw: vec![false, true],
        }
    }

    fn unit(ty: u8, class: u32, mode: u32) -> ClientUnit {
        let mut u = ClientUnit::new(UnitKey::new(ty, 1));
        u.class = class;
        u.mode = mode;
        u
    }

    // Covers: specs/render/draw-order.md §3 r4
    #[test]
    fn dead_unflat_dead_monster_is_not_flat() {
        let w = ClientWorld::default();
        let t = tables();
        let flat = model_facts(&w, &unit(MONSTER, 0, 12), &t, None).unwrap();
        assert!(is_flat(&flat));
        let upright = model_facts(&w, &unit(MONSTER, 1, 12), &t, None).unwrap();
        assert!(upright.unflat_dead && !is_flat(&upright));
        assert!(model_facts(&w, &unit(MONSTER, 2, 12), &t, None).is_err());
    }

    // Covers: specs/render/draw-order.md §3 r4
    #[test]
    fn draw_under_object_is_flat() {
        let w = ClientWorld::default();
        let t = tables();
        let under = model_facts(&w, &unit(OBJECT, 1, 0), &t, None).unwrap();
        assert_eq!(under.draw_under, 2);
        assert!(is_flat(&under));
        // DrawUnder bit 1 only in mode 2.
        assert!(!is_flat(
            &model_facts(&w, &unit(OBJECT, 2, 0), &t, None).unwrap()
        ));
        assert!(is_flat(
            &model_facts(&w, &unit(OBJECT, 2, 2), &t, None).unwrap()
        ));
        assert!(!is_flat(
            &model_facts(&w, &unit(OBJECT, 0, 2), &t, None).unwrap()
        ));
        assert!(model_facts(&w, &unit(OBJECT, 9, 0), &t, None).is_err());
    }

    // Covers: specs/render/draw-order.md §5 r3
    #[test]
    fn states_flags_and_flag_ex_come_from_the_model() {
        let w = ClientWorld::default();
        let t = tables();
        let mut u = unit(PLAYER, 0, 17);
        u.states.extend([STATE_PLAYERBODY, STATE_INVIS]);
        u.flag_ex = 0x2000_0000;
        u.flag_200 = true;
        u.flag_2 = Some(true);
        let f = model_facts(&w, &u, &t, None).unwrap();
        assert!(f.playerbody && f.invis && !f.attached);
        assert_eq!(f.flags, FLAG_200 | FLAG_2);
        assert_eq!(f.flag_ex, 0x2000_0000);
        assert_eq!(f.flag_ex & UNIT_EX_VISIBLE, 0);
        u.quest_untargetable = true;
        assert_eq!(flags_of(&u), FLAG_200);
    }
}
