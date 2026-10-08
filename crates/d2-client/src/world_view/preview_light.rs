// Spec: specs/render/lighting.md (§1–§3, §7.2, §9, §11 r1–r4, §8 player row), specs/render/shading.md (§3)
//! The play preview's lighting (stitch-lighting; replaces the D1
//! full-bright fill of [`super::preview`] unless `D2RS_FULLBRIGHT=1`).
//!
//! Per drawn frame ([`PreviewLight::refresh`]): the 48 × 48 light map
//! around the local player (§1), the ambient fill from the act
//! environment the bridge steps per client update (§3, §9.2 r1), the player's light record (radius 13 plus stat
//! 89, §8) and the other records of the client light list, each plain
//! (§7.2); then the cel light value of a unit (§11 r1) and of a tile
//! ([`tile_chain`]) is read from it, and shaded through the act's PL2
//! tables.
//!
//! `d2rs-own, unverified`:
//! - a tile's whole-tile shade is one flat value (the cell at its centre
//!   sub-tile); its blocks take the gradients of [`super::preview_blocks`];
//! - the ambient of a room is the scripted override (§10, quest byte 1 read
//!   as 0), else the level's `Levels.txt` ambient when it has a colour,
//!   else the environment's; the near rooms fill their rectangles (§3 r3);
//! - the other lights are those of [`super::light_sources`];
//! - the blocks-light flags (§4) come from the client DRLG collision
//!   (`collision_at`, mask 0x22); only the player's light (kind 0) is
//!   shadowed, the other sources draw plain (REC-250);
//! - the other units' lights sit at their sub-tile's `8·s + 4` (the model
//!   holds no precise position); the local player's at `(P >> 13) + 4` of
//!   its predicted 16.16 position (§6.1).

use crate::bridge::world::ClientWorld;
use crate::bridge::{ClientUnit, UnitKey};
use crate::composite::ComponentRequest;
use crate::rules::draw_order::{Dt1Facts, TileKind};
use crate::rules::lighting::contribute;
use crate::rules::lighting::draws::MATERIAL_UNLIT;
use crate::rules::lighting::environment::{Ambient as EnvAmbient, Environment, PeriodTables};
use crate::rules::lighting::map::{Ambient, AmbientScene, LightMap, NearRoom};
use crate::rules::lighting::records::{LightKind, LightList};
use crate::rules::lighting::sources::{player_light_color, player_light_radius};
use crate::rules::lighting::view::{ComponentLook, FrameLight, LookFeed};
use crate::rules::shading::ShadeTables;
use crate::scene::ShadeChain;

/// Stat 89 `item_lightradius` and 90 `item_lightcolor` (§8 player row).
pub const STAT_LIGHT_RADIUS: u16 = 89;
pub const STAT_LIGHT_COLOR: u16 = 90;

/// Light quality used by the preview: not 0, so no radius caps (§7.2).
const QUALITY: u8 = 2;

/// The collision mask of the blocks-light test (§4: bits 0x02 and 0x20).
pub const BLOCKS_LIGHT_MASK: u16 = 0x22;

/// The blocks-light flag of a sub-tile from its collision mask (§4):
/// mask 0x22 non-zero; a sub-tile in no loaded room (`None`) blocks light
/// (§4 r2: the point test returns 0x27 unmasked).
pub fn blocks_light(collision: Option<u16>) -> bool {
    collision.is_none_or(|m| m & BLOCKS_LIGHT_MASK != 0)
}

/// Environment variable that turns the lighting off (full bright, D1).
pub const FULLBRIGHT_VAR: &str = "D2RS_FULLBRIGHT";

/// Whether the debug switch asks for full bright.
pub fn fullbright_requested() -> bool {
    std::env::var(FULLBRIGHT_VAR).is_ok_and(|v| !v.is_empty() && v != "0")
}

/// The sub-tile of a 16.16 position.
pub fn subtile_of(x16: u32, y16: u32) -> (i32, i32) {
    ((x16 >> 16) as i32, (y16 >> 16) as i32)
}

/// The `LookFeed` of the preview: a unit's light is the cell at its
/// sub-tile; the local player is at its predicted one; no ghostly, no
/// override, hover only from the cursor pick, no remap (`d2rs-own, unverified`).
#[derive(Debug, Clone, Default)]
pub struct PreviewLook {
    pub local: Option<(UnitKey, (i32, i32))>,
    /// The `colorpri` / `colorshift` of the states (`state_tint`, PROVISIONAL
    /// REC-245); none: no unit is tinted.
    pub tints: Option<std::sync::Arc<super::state_tint::StateTints>>,
    /// The act tables of the frame, for the tint's remap map.
    pub tables: Option<ShadeTables>,
    /// The unit under the cursor (drawn highlighted, `blend-modes.md` §3).
    pub hover: Option<UnitKey>,
}

impl PreviewLook {
    /// Whether `unit` is the hover target (drawn highlighted).
    pub fn is_hovered(&self, unit: &ClientUnit) -> bool {
        self.hover == Some(unit.key)
    }
}

impl LookFeed for PreviewLook {
    fn light_subtile(&self, unit: &ClientUnit) -> Result<(i32, i32), String> {
        if let Some((key, at)) = self.local {
            if key == unit.key {
                return Ok(at);
            }
        }
        let (x, y) = unit.position.unwrap_or((0, 0));
        Ok((i32::from(x), i32::from(y)))
    }

    fn look(&self, unit: &ClientUnit, _: &ComponentRequest<'_>) -> Result<ComponentLook, String> {
        Ok(ComponentLook {
            ghostly: false,
            override_input: None,
            hovered: self.is_hovered(unit),
            remap: self
                .tints
                .as_ref()
                .and_then(|t| t.remap(&unit.states, self.tables.as_ref())),
        })
    }
}

/// The frame's light state of the preview.
#[derive(Debug, Clone, Default)]
pub struct PreviewLight {
    /// `D2RS_FULLBRIGHT=1`: no light is built, the D1 fill stays.
    pub fullbright: bool,
    periods: Option<PeriodTables>,
    /// The act environment's ambient of the frame (roof tiles, §11 r4).
    env_cell: crate::rules::lighting::map::LightCell,
    frame: Option<FrameLight>,
    look: PreviewLook,
    /// The monster / missile light columns ([`super::light_sources`]);
    /// objects from the `objects` rows. `None`: only the player's light.
    pub sources: Option<std::sync::Arc<super::light_sources::LightRows>>,
}

/// The ambient of a level (§3.1 r2): its `Levels.txt` `Intensity`, `Red`,
/// `Green`, `Blue` when any colour byte is non-zero, else `None` (the act
/// environment applies, r3).
pub fn level_ambient(rows: &[(u8, u8, u8, u8)], level: u32) -> Option<Ambient> {
    let &(i, r, g, b) = rows.get(level as usize)?;
    (r != 0 || g != 0 || b != 0).then_some(Ambient { i, r, g, b })
}

/// A light: sub-tile, radius, rgb.
pub type PointLight = ((i32, i32), i32, (u8, u8, u8));

/// The light map of a frame: ambient fill, then each `(sub-tile, radius,
/// rgb)` light plain (§2 r2, r4; §7.2).
pub fn build_map(player: (i32, i32), ambient: Ambient, lights: &[PointLight]) -> LightMap {
    let scene = AmbientScene {
        player_ambient: ambient,
        near: Vec::new(),
    };
    build_map_blocked(player, &scene, |_, _| false, lights, false)
}

/// The light map of a frame with the ambient scene (§3 r3 near-room
/// fills) and the blocks-light flags (§4, `blocks` = the collision point
/// test with mask 0x22 at a sub-tile). With `shadow_first` the first light
/// (the local player's, kind 0, §8) takes the shadowed contribution (§7.3,
/// quality 2); the others draw plain.
pub fn build_map_blocked(
    player: (i32, i32),
    scene: &AmbientScene,
    blocks: impl FnMut(i32, i32) -> bool,
    lights: &[PointLight],
    shadow_first: bool,
) -> LightMap {
    // A unit standing on a sub-tile's origin: `(P >> 13) + 4` = 8·s + 4
    // (§6.1).
    let lights: Vec<PointLight> = lights
        .iter()
        .map(|&((sx, sy), r, rgb)| ((8 * sx + 4, 8 * sy + 4), r, rgb))
        .collect();
    build_map_eighths(player, scene, blocks, &lights, shadow_first)
}

/// The light position of a unit at 16.16 position `(x16, y16)` (§6.1,
/// `0x006203B0`): `(P >> 13) + 4` on each axis, in 1/8 sub-tile.
pub fn light_pos_of(x16: u32, y16: u32) -> (i32, i32) {
    (
        crate::rules::lighting::records::unit_light_pos(x16 as i32),
        crate::rules::lighting::records::unit_light_pos(y16 as i32),
    )
}

/// [`build_map_blocked`] with each light's position already in 1/8
/// sub-tile (§6.1: `(P >> 13) + 4` of the unit's precise position).
pub fn build_map_eighths(
    player: (i32, i32),
    scene: &AmbientScene,
    blocks: impl FnMut(i32, i32) -> bool,
    lights: &[PointLight],
    shadow_first: bool,
) -> LightMap {
    let mut map = LightMap::new(player);
    map.fill_ambient(Some(scene));
    map.fill_blocks(blocks);
    let mut list = LightList::new();
    for (n, &((px, py), r, (red, green, blue))) in lights.iter().enumerate() {
        let kind = if shadow_first && n == 0 {
            LightKind::Shadowed
        } else {
            LightKind::Plain
        };
        list.create(None, (px, py), kind, r, 255, red, green, blue);
    }
    let colored = list.colored;
    for (_, rec) in list.iter() {
        match rec.kind {
            LightKind::Shadowed => contribute::shadowed(&mut map, rec, colored),
            _ => contribute::plain(&mut map, rec, QUALITY, false, colored),
        }
    }
    map
}

/// The light byte of a tile (§11 r3, r4 as the preview reads them): the
/// cell at its centre sub-tile; an unlit floor (material 0x100) is 0xFF; a
/// roof with height ≠ 0 takes the environment intensity.
pub fn tile_light_byte(
    map: &LightMap,
    ambient_i: u8,
    kind: TileKind,
    dt1: &Dt1Facts,
    cell: (i32, i32),
) -> u8 {
    match kind {
        TileKind::Floor { .. } if u32::from(dt1.material) & MATERIAL_UNLIT != 0 => 0xFF,
        TileKind::Roof { .. } if dt1.roof_height != 0 => ambient_i,
        _ => {
            let (sx, sy) = (5 * cell.0 + 2, 5 * cell.1 + 2);
            map.read(8 * sx, 8 * sy).i
        }
    }
}

/// The shade chain of a flat light byte (`shading.md` §3 r1).
pub fn chain_of(tables: &ShadeTables, v: u8) -> ShadeChain {
    match tables.cel_light(v) {
        Some(m) => ShadeChain::new(&[m]).expect("one map"),
        None => ShadeChain::EMPTY,
    }
}

impl PreviewLight {
    /// The ambient of a room of `level` (§3.1): the scripted override
    /// (§10) when it has a colour, else the level's own, else the act's
    /// `env`.
    fn room_ambient(&self, world: &ClientWorld, level: u32, env: Ambient) -> Ambient {
        // d2rs-own, unverified (REC-250): client quest byte 1 is not held,
        // read as 0 (the Den of Evil glow stays off).
        let o = world.overrides.ambient(level, 0);
        if o.r != 0 || o.g != 0 || o.b != 0 {
            return Ambient {
                i: o.i,
                r: o.r,
                g: o.g,
                b: o.b,
            };
        }
        self.sources
            .as_ref()
            .and_then(|s| level_ambient(&s.levels, level))
            .unwrap_or(env)
    }

    /// The ambient input of §3: the player room's ambient and the near
    /// list (its adjacency array, `rooms.md` §6), each with its own level
    /// ambient and sub-tile rectangle.
    fn scene(&self, world: &ClientWorld, env: Ambient, level: u32) -> AmbientScene {
        let mut near = Vec::new();
        if let (Some(own), Some(drlg), Some(active)) = (
            world.local_room(),
            world.drlg.as_ref(),
            world.active_rooms.as_ref(),
        ) {
            for id in drlg.adjacency(own.room) {
                let Some(r) = active.iter().find(|a| a.room == id) else {
                    continue;
                };
                near.push(NearRoom {
                    rect: (r.x0, r.y0, r.w, r.h),
                    ambient: self.room_ambient(world, u32::from(r.level), env),
                    is_player_room: id == own.room,
                });
            }
        }
        AmbientScene {
            player_ambient: self.room_ambient(world, level, env),
            near,
        }
    }

    pub fn new() -> Self {
        PreviewLight {
            fullbright: fullbright_requested(),
            ..PreviewLight::default()
        }
    }

    /// The frame's light, `None` in full bright or before the first
    /// refresh.
    pub fn frame(&self) -> Option<&FrameLight> {
        if self.fullbright {
            return None;
        }
        self.frame.as_ref()
    }

    /// The unit under the cursor this frame.
    pub fn set_hover(&mut self, unit: Option<UnitKey>) {
        self.look.hover = unit;
    }

    /// The state tints of the tables (`state_tint`).
    pub fn set_tints(&mut self, tints: Option<std::sync::Arc<super::state_tint::StateTints>>) {
        self.look.tints = tints;
    }

    pub fn look(&self) -> &PreviewLook {
        &self.look
    }

    /// Rebuilds the frame's light (once per drawn frame, before the build).
    /// `local_at` is the predicted 16.16 position of the local player.
    pub fn refresh(
        &mut self,
        world: &ClientWorld,
        local_at: Option<(UnitKey, (u32, u32))>,
        tables: Option<&ShadeTables>,
    ) {
        self.frame = None;
        self.look.local = None;
        self.look.tables = tables.copied();
        let (Some(tables), false) = (tables, self.fullbright) else {
            return;
        };
        let Some(player) = world.local() else {
            return;
        };
        let (at, at8) = match local_at {
            Some((key, (x, y))) if key == player.key => (subtile_of(x, y), light_pos_of(x, y)),
            _ => {
                let at = player
                    .position
                    .map_or((0, 0), |(x, y)| (i32::from(x), i32::from(y)));
                (at, (8 * at.0 + 4, 8 * at.1 + 4))
            }
        };
        self.look.local = Some((player.key, at));
        let level = world.player_level().map_or(0, u32::from);
        if self.periods.is_none() {
            self.periods = PeriodTables::builtin().ok();
        }
        let Some(periods) = self.periods else {
            return;
        };
        // The model's record (§9.1), stepped once per client update by the
        // bridge (§9.2 r1) and set by S→C 0x53 / 0x5D (r2–r4); the map
        // build only reads it. Before the act's record exists: a fresh
        // one (creation values).
        let env = world
            .environment
            .unwrap_or_else(|| Environment::new(&periods, 0));
        let a: EnvAmbient = env.ambient();
        let ambient = Ambient {
            i: a.i,
            r: a.r,
            g: a.g,
            b: a.b,
        };
        self.env_cell = crate::rules::lighting::map::LightCell {
            blocks: 0,
            i: a.i,
            r: a.r,
            g: a.g,
            b: a.b,
        };
        let scene = self.scene(world, ambient, level);
        let radius = player_light_radius(player.stat(STAT_LIGHT_RADIUS)).max(1);
        let rgb = player_light_color(player.stat(STAT_LIGHT_COLOR) as u32);
        let mut lights = vec![(at8, radius, rgb)];
        if let Some(rows) = &self.sources {
            lights.extend(
                rows.lights(world)
                    .into_iter()
                    .map(|((sx, sy), r, c)| ((8 * sx + 4, 8 * sy + 4), r, c)),
            );
        }
        let drlg = world.drlg.as_ref().filter(|_| world.local_room().is_some());
        let map = build_map_eighths(
            at,
            &scene,
            |x, y| drlg.is_some_and(|d| blocks_light(d.drlg.collision_at(x, y))),
            &lights,
            true,
        );
        self.frame = Some(FrameLight {
            tables: *tables,
            map,
        });
    }

    /// The per-block shades of a tile (§11 r2–r4, `shading.md` §4); empty
    /// in full bright or when the tile keeps its flat shade. `fade`: the
    /// record's alpha byte and fade state.
    #[allow(clippy::too_many_arguments)]
    pub fn block_shades(
        &self,
        kind: TileKind,
        dt1: &Dt1Facts,
        cell: (i32, i32),
        fade: (u8, u8),
        blend: crate::scene::BlendOp,
        blocks: &[crate::rules::BlockRect],
        grids: &[(u8, u8)],
    ) -> Vec<crate::rules::BlockShade> {
        let Some(f) = self.frame() else {
            return Vec::new();
        };
        super::preview_blocks::block_shades(
            f,
            self.env_cell,
            kind,
            dt1,
            cell,
            fade,
            blend,
            blocks,
            grids,
        )
    }

    /// The flat shade of a tile, `None` in full bright.
    pub fn tile_chain(
        &self,
        kind: TileKind,
        dt1: &Dt1Facts,
        cell: (i32, i32),
    ) -> Option<ShadeChain> {
        let f = self.frame()?;
        let v = tile_light_byte(&f.map, self.env_cell.i, kind, dt1, cell);
        Some(chain_of(&f.tables, v))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bridge::world::PLAYER;
    use crate::scene::MapTable;

    fn tables() -> ShadeTables {
        let pl2 =
            d2_formats::palette::Pl2::parse(&super::super::tile_assets::tests::pl2()).unwrap();
        ShadeTables::push(&mut MapTable::new(), &pl2)
    }

    fn dark() -> Ambient {
        Ambient {
            i: 32,
            r: 32,
            g: 32,
            b: 32,
        }
    }

    // Covers: specs/render/lighting.md §7.2, §11 r3
    #[test]
    fn tile_light_follows_the_radius_and_drops_outside_it() {
        let player = (100, 100);
        let map = build_map(player, dark(), &[(player, 6, (255, 255, 255))]);
        let dt1 = Dt1Facts::default();
        let floor = TileKind::Floor { layer: 1 };
        // Tile (20, 20): centre sub-tile 102, two from the player.
        let near = tile_light_byte(&map, 32, floor, &dt1, (20, 20));
        let far = tile_light_byte(&map, 32, floor, &dt1, (20 + 4, 20));
        assert!(near > 32 + 40, "near the light: {near}");
        assert_eq!(far, 32, "outside the radius: ambient");
        let t = tables();
        assert_ne!(chain_of(&t, near), chain_of(&t, far));
        assert_eq!(chain_of(&t, 0xFF), ShadeChain::EMPTY);
    }

    // Covers: specs/render/blend-modes.md §3
    #[test]
    fn only_the_hover_target_is_highlighted() {
        let (a, b) = (UnitKey::new(PLAYER, 1), UnitKey::new(PLAYER, 2));
        let mut light = PreviewLight::default();
        light.set_hover(Some(b));
        assert!(!light.look().is_hovered(&ClientUnit::new(a)));
        assert!(light.look().is_hovered(&ClientUnit::new(b)));
        light.set_hover(None);
        assert!(!light.look().is_hovered(&ClientUnit::new(b)));
    }

    // Covers: specs/render/lighting.md §11 r4
    #[test]
    fn unlit_floor_and_high_roof_are_special() {
        let map = build_map((50, 50), dark(), &[]);
        let unlit = Dt1Facts {
            material: 0x100,
            ..Dt1Facts::default()
        };
        assert_eq!(
            tile_light_byte(&map, 77, TileKind::Floor { layer: 1 }, &unlit, (10, 10)),
            0xFF
        );
        let roof = Dt1Facts {
            roof_height: 3,
            ..Dt1Facts::default()
        };
        assert_eq!(
            tile_light_byte(&map, 77, TileKind::Roof { pass: 1 }, &roof, (10, 10)),
            77
        );
    }

    // Covers: specs/render/lighting.md §11 r1
    #[test]
    fn refresh_lights_the_player_and_darkens_far_cells() {
        let mut w = ClientWorld::default();
        let key = UnitKey::new(PLAYER, 1);
        let mut u = ClientUnit::new(key);
        u.position = Some((4000, 4000));
        w.units.insert(key, u);
        w.local_player = Some(key);
        let t = tables();
        let mut light = PreviewLight::default();
        light.refresh(&w, None, Some(&t));
        let f = light.frame().expect("frame light");
        let at = f.map.read(8 * 4000, 8 * 4000).i;
        let far = f.map.read(8 * (4000 + 20), 8 * 4000).i;
        assert!(at > far, "lit {at} > outside the radius {far}");
        // The unit's light is the cell at its sub-tile.
        let unit = w.units.get(&key).unwrap();
        assert_eq!(light.look().light_subtile(unit).unwrap(), (4000, 4000));
        // Stat 89 widens the radius.
        let mut w2 = w.clone();
        w2.units.get_mut(&key).unwrap().stats.insert(89, 4);
        let mut wide = PreviewLight::default();
        wide.refresh(&w2, None, Some(&t));
        let edge = (4000 + 16, 4000);
        assert!(
            wide.frame().unwrap().map.read(8 * edge.0, 8 * edge.1).i
                > f.map.read(8 * edge.0, 8 * edge.1).i
        );
    }

    // Covers: specs/render/lighting.md §4 r2, §4 r3
    #[test]
    fn a_cell_in_no_room_blocks_light() {
        assert!(blocks_light(None));
        assert!(blocks_light(Some(0x02)));
        assert!(blocks_light(Some(0x20)));
        assert!(!blocks_light(Some(0x01 | 0x04 | 0x10 | 0x40)));
        assert!(!blocks_light(Some(0)));
    }

    // Covers: specs/render/lighting.md §6.1
    // (position `(P >> 13) + 4`; test vector: sub-tile 100, fraction 0 → 804)
    #[test]
    fn the_player_light_sits_at_its_precise_position() {
        assert_eq!(light_pos_of(100 << 16, 100 << 16), (804, 804));
        // Three quarters into sub-tile 100: 800 + 6 + 4.
        assert_eq!(light_pos_of((100 << 16) + 0xC000, 100 << 16), (810, 804));
        let mut w = ClientWorld::default();
        let key = UnitKey::new(PLAYER, 1);
        let mut u = ClientUnit::new(key);
        u.position = Some((100, 100));
        w.units.insert(key, u);
        w.local_player = Some(key);
        let t = tables();
        let map = |x16: u32| {
            let mut l = PreviewLight::default();
            l.refresh(&w, Some((key, (x16, 100 << 16))), Some(&t));
            l.frame().unwrap().map.clone()
        };
        let (whole, frac) = (map(100 << 16), map((100 << 16) + 0xC000));
        // §7.1 r4: cell 110's corner 880 is 76 (whole) or 70 (frac) away,
        // cell 90's corner 720 is 84 or 90: the light moved right.
        let i = |m: &LightMap, sx: i32| m.read(8 * sx, 8 * 100).i;
        assert!(i(&frac, 110) > i(&whole, 110));
        assert!(i(&frac, 90) < i(&whole, 90));
    }

    // Covers: specs/render/lighting.md §9.2 r2
    // (the 0x53 setter's record is what the ambient reads)
    #[test]
    fn a_new_environment_record_reaches_the_ambient() {
        let mut w = ClientWorld::default();
        let key = UnitKey::new(PLAYER, 1);
        let mut u = ClientUnit::new(key);
        u.position = Some((4000, 4000));
        w.units.insert(key, u);
        w.local_player = Some(key);
        let t = tables();
        let far = (8 * 4000 + 8 * 40, 8 * 4000);
        let mut light = PreviewLight::default();
        light.refresh(&w, None, Some(&t));
        assert_eq!(light.frame().unwrap().map.read(far.0, far.1).i, 128);
        // Noon (90 degrees, index 2): §9.3 r4 gives 255.
        let periods = PeriodTables::builtin().unwrap();
        let mut env = Environment::new(&periods, 0);
        env.ticks = 90 * 128;
        // As the bridge's client update leaves it (§9.2 r1): the map
        // build reads the record, it no longer steps it.
        env.update(&periods, 0);
        w.environment = Some(env);
        light.refresh(&w, None, Some(&t));
        assert_eq!(light.frame().unwrap().map.read(far.0, far.1).i, 255);
    }

    // Covers: specs/render/lighting.md §9.2 r1
    #[test]
    fn drawn_frames_do_not_step_the_environment() {
        let mut w = ClientWorld::default();
        let key = UnitKey::new(PLAYER, 1);
        let mut u = ClientUnit::new(key);
        u.position = Some((4000, 4000));
        w.units.insert(key, u);
        w.local_player = Some(key);
        let t = tables();
        let far = (8 * 4000 + 8 * 40, 8 * 4000);
        let periods = PeriodTables::builtin().unwrap();
        // Mid-morning: the intensity changes with every update.
        let mut env = Environment::new(&periods, 0);
        env.ticks = 45 * 128;
        env.update(&periods, 0);
        w.environment = Some(env);
        let mut light = PreviewLight::default();
        // More drawn frames than one degree's ticks (speed 128): per-frame
        // stepping would move the intensity.
        let mut stepped = env;
        for _ in 0..200 {
            stepped.update(&periods, 0);
        }
        assert_ne!(stepped.ambient().i, env.ambient().i);
        for _ in 0..200 {
            light.refresh(&w, None, Some(&t));
            assert_eq!(
                light.frame().unwrap().map.read(far.0, far.1).i,
                env.ambient().i
            );
        }
    }

    // Covers: specs/render/lighting.md §3.1 r2
    #[test]
    fn the_levels_ambient_replaces_the_environments_when_it_has_a_colour() {
        let mut w = ClientWorld::default();
        let key = UnitKey::new(PLAYER, 1);
        let mut u = ClientUnit::new(key);
        u.position = Some((4000, 4000));
        w.units.insert(key, u);
        w.local_player = Some(key);
        let t = tables();
        let far = (8 * 4000 + 8 * 40, 8 * 4000);
        let mut plain = PreviewLight::default();
        plain.refresh(&w, None, Some(&t));
        // Level 0 (no room): the row's own ambient needs a colour.
        let rows = super::super::light_sources::LightRows {
            levels: vec![(99, 255, 255, 255)],
            ..Default::default()
        };
        let mut lit = PreviewLight {
            sources: Some(std::sync::Arc::new(rows)),
            ..PreviewLight::default()
        };
        lit.refresh(&w, None, Some(&t));
        assert_eq!(lit.frame().unwrap().map.read(far.0, far.1).i, 99);
        assert_ne!(plain.frame().unwrap().map.read(far.0, far.1).i, 99);
    }

    // Covers: specs/render/lighting.md §8
    #[test]
    fn a_monster_light_lights_its_surroundings() {
        use crate::bridge::world::MONSTER;
        let mut w = ClientWorld::default();
        let key = UnitKey::new(PLAYER, 1);
        let mut u = ClientUnit::new(key);
        u.position = Some((4000, 4000));
        w.units.insert(key, u);
        w.local_player = Some(key);
        let mk = UnitKey::new(MONSTER, 9);
        let mut m = ClientUnit::new(mk);
        m.class = 0;
        m.mode = 1;
        m.position = Some((4000, 4018));
        w.units.insert(mk, m);
        let t = tables();
        let probe = (8 * 4000, 8 * 4018);
        let mut plain = PreviewLight::default();
        plain.refresh(&w, None, Some(&t));
        let dark = plain.frame().unwrap().map.read(probe.0, probe.1).i;
        let rows = super::super::light_sources::LightRows {
            monsters: vec![(6, (255, 255, 255))],
            ..Default::default()
        };
        let mut lit = PreviewLight {
            sources: Some(std::sync::Arc::new(rows)),
            ..PreviewLight::default()
        };
        lit.refresh(&w, None, Some(&t));
        assert!(lit.frame().unwrap().map.read(probe.0, probe.1).i > dark);
    }

    // Covers: specs/render/lighting.md §3
    #[test]
    fn fullbright_gives_no_frame_light() {
        let mut w = ClientWorld::default();
        let key = UnitKey::new(PLAYER, 1);
        w.units.insert(key, ClientUnit::new(key));
        w.local_player = Some(key);
        let t = tables();
        let mut light = PreviewLight {
            fullbright: true,
            ..PreviewLight::default()
        };
        light.refresh(&w, None, Some(&t));
        assert!(light.frame().is_none());
        assert!(light
            .tile_chain(TileKind::Wall, &Dt1Facts::default(), (1, 1))
            .is_none());
    }

    // Covers: specs/render/lighting.md §11 r1
    #[test]
    fn the_model_feed_states_the_light_unless_fullbright() {
        use crate::world_view::feed::{NoFeed, ViewFeed};
        use crate::world_view::model_feed::ModelFeed;
        use crate::world_view::preview::Preview;
        let mut w = ClientWorld::default();
        let key = UnitKey::new(PLAYER, 1);
        let mut u = ClientUnit::new(key);
        u.position = Some((900, 900));
        w.units.insert(key, u);
        w.local_player = Some(key);
        let t = tables();
        let mut feed = ModelFeed::<NoFeed>::default().with_preview(Preview::default());
        assert!(feed.light(&w).unwrap().is_none(), "before the first frame");
        feed.preview
            .as_mut()
            .unwrap()
            .light
            .refresh(&w, None, Some(&t));
        let l = feed.light(&w).unwrap().expect("lit");
        assert!(l.light.map.read(8 * 900, 8 * 900).i > l.light.map.read(8 * 940, 8 * 900).i);
        feed.preview.as_mut().unwrap().light.fullbright = true;
        feed.preview
            .as_mut()
            .unwrap()
            .light
            .refresh(&w, None, Some(&t));
        assert!(feed.light(&w).unwrap().is_none());
    }
}

#[cfg(test)]
mod block_feed_tests {
    use crate::rules::{BlockRect, BlockShade, MapTile, ViewSource};
    use crate::scene::{BlendOp, DrawKey, ShadeChain};
    use crate::world_view::feed::NoFeed;
    use crate::world_view::model_feed::ModelFeed;
    use crate::world_view::preview::Preview;

    // Covers: specs/render/lighting.md §11 r2
    #[test]
    fn the_model_feed_answers_the_previews_block_shades() {
        let preview = Preview::default();
        let key = DrawKey::new(1, 2, 3, 0).unwrap();
        let block = BlockShade {
            block: BlockRect {
                x: 0,
                y: 0,
                width: 32,
                height: 32,
            },
            shade: ShadeChain::EMPTY,
            blend: BlendOp::Opaque,
        };
        preview.put_block_shades(key, vec![block]);
        let feed = ModelFeed::<NoFeed>::default().with_preview(preview);
        let tile = |key| MapTile {
            cell: (1, 1),
            list: crate::rules::camera::TileList::Floor,
            frame: crate::composite::ComponentFrame {
                set: crate::world_view::preview::skip_key(),
                index: 0,
            },
            blocks: Vec::new(),
            shade: ShadeChain::EMPTY,
            blend: BlendOp::Opaque,
            key,
        };
        assert_eq!(feed.tile_blocks(&tile(key)).unwrap(), vec![block]);
        let other = DrawKey::new(1, 2, 4, 0).unwrap();
        assert!(feed.tile_blocks(&tile(other)).unwrap().is_empty());
    }
}
