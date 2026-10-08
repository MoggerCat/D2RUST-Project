// Spec: specs/render/lighting.md (§8 monster, object and missile rows)
//! The light records of the units other than the local player, for the play
//! preview's light map (q-light-radius-detail).
//!
//! `d2rs-own, unverified`:
//! - the records are rebuilt every frame from the model's units, as plain
//!   lights (§7.2, kind 0 / 1 / 2 alike), not kept in the client list, so
//!   no radius walk (§6.4), flicker, target radius, umod 3 light, skill
//!   cast light, overlay light or Den of Evil light;
//! - a monster's radius is `monstats2` `Light` (no component `L_c`, no level
//!   8 quest override); a dead monster has none;
//! - an object's radius is `Lit<mode>` / 2 with the `objects` colour;
//! - a missile's radius is `Light` with the `missiles` colour; none when
//!   `Light` is 0 (the high-quality gate of §8 is taken as on).

use d2_data::tables::{Missiles, Monstats, Monstats2, Objects};

use crate::bridge::world::{ClientWorld, MISSILE, MONSTER, OBJECT};
use crate::world_view::preview_light::PointLight;

/// `Light` radius and `Red`, `Green`, `Blue` of a row.
type Row = (u8, (u8, u8, u8));

/// The per-class light columns of the monster and missile tables.
#[derive(Debug, Clone, Default)]
pub struct LightRows {
    /// By `monstats` row (the unit class).
    pub monsters: Vec<Row>,
    /// By `missiles` row (the missile id).
    pub missiles: Vec<Row>,
    /// By `objects` row: `Lit0`…`Lit7` and the colour.
    pub objects: Vec<([u8; 8], (u8, u8, u8))>,
}

impl LightRows {
    pub fn from_tables(
        monstats: &[Monstats],
        monstats2: &[Monstats2],
        missiles: &[Missiles],
        objects: &[Objects],
    ) -> Self {
        LightRows {
            monsters: monstats
                .iter()
                .map(|m| {
                    monstats2
                        .get(usize::from(m.monstatsex))
                        .map_or((0, (0, 0, 0)), |x| {
                            (x.light, (x.light_r, x.light_g, x.light_b))
                        })
                })
                .collect(),
            missiles: missiles
                .iter()
                .map(|m| (m.light, (m.red, m.green, m.blue)))
                .collect(),
            objects: objects
                .iter()
                .map(|o| {
                    (
                        [
                            o.lit0, o.lit1, o.lit2, o.lit3, o.lit4, o.lit5, o.lit6, o.lit7,
                        ],
                        (o.red, o.green, o.blue),
                    )
                })
                .collect(),
        }
    }

    /// The lights of the world's non-player units, as sub-tile lights
    /// (`radius` in sub-tiles, §8). Objects come from the `objects` rows.
    pub fn lights(&self, world: &ClientWorld) -> Vec<PointLight> {
        let mut out = Vec::new();
        for unit in world.units.values() {
            let (x, y) = unit.position.unwrap_or((0, 0));
            let at = (i32::from(x), i32::from(y));
            let class = unit.class as usize;
            let (radius, rgb) = match unit.key.unit_type {
                MONSTER if !unit.is_dead() => self.monsters.get(class).copied().unwrap_or_default(),
                MISSILE => self.missiles.get(class).copied().unwrap_or_default(),
                OBJECT => match self.objects.get(class) {
                    Some((lit, rgb)) => (lit.get(unit.mode as usize).map_or(0, |l| l / 2), *rgb),
                    None => continue,
                },
                _ => continue,
            };
            if radius != 0 {
                out.push((at, i32::from(radius), rgb));
            }
        }
        out
    }
}

/// The light columns of the user's tables.
pub fn load(archives: &dyn d2_data::bin::TableFiles) -> Result<LightRows, String> {
    use d2_data::tables::{decode_all, Record};
    let set = d2_data::bin::load_from(archives, "eng").map_err(|e| e.to_string())?;
    fn all<R: Record>(set: &d2_data::bin::BinSet, name: &str) -> Result<Vec<R>, String> {
        match set.table(name) {
            Some(t) => decode_all(t).map_err(|e| e.to_string()),
            None => Ok(Vec::new()),
        }
    }
    Ok(LightRows::from_tables(
        &all::<Monstats>(&set, "monstats")?,
        &all::<Monstats2>(&set, "monstats2")?,
        &all::<Missiles>(&set, "missiles")?,
        &all::<Objects>(&set, "objects")?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bridge::world::{ClientUnit, PLAYER};
    use crate::bridge::UnitKey;

    fn unit(world: &mut ClientWorld, ty: u8, guid: u32, class: u32, mode: u32, at: (u16, u16)) {
        let key = UnitKey::new(ty, guid);
        let mut u = ClientUnit::new(key);
        u.class = class;
        u.mode = mode;
        u.position = Some(at);
        world.units.insert(key, u);
    }

    // Covers: specs/render/lighting.md §8
    #[test]
    fn monsters_objects_and_missiles_give_lights() {
        let rows = LightRows {
            objects: vec![([0, 0, 12, 0, 0, 0, 0, 0], (9, 8, 7))],
            monsters: vec![(0, (0, 0, 0)), (5, (230, 168, 255))],
            missiles: vec![(8, (1, 2, 3))],
        };
        let mut w = ClientWorld::default();
        unit(&mut w, MONSTER, 1, 1, 1, (100, 100));
        unit(&mut w, MONSTER, 2, 0, 1, (101, 100));
        unit(&mut w, MONSTER, 3, 1, 0xC, (102, 100));
        unit(&mut w, MISSILE, 4, 0, 1, (110, 100));
        unit(&mut w, OBJECT, 5, 0, 2, (120, 100));
        unit(&mut w, OBJECT, 6, 0, 0, (121, 100));
        unit(&mut w, PLAYER, 7, 0, 1, (130, 100));
        let l = rows.lights(&w);
        assert_eq!(l.len(), 3, "{l:?}");
        assert!(l.contains(&((100, 100), 5, (230, 168, 255))));
        assert!(l.contains(&((110, 100), 8, (1, 2, 3))));
        assert!(l.contains(&((120, 100), 6, (9, 8, 7))));
    }
}
