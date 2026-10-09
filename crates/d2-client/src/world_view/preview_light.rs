// Spec: specs/render/lighting.md (§1–§3, §6.4, §7.2–§7.4, §9, §10 r1, §11 r1–r4, §8 player row), specs/render/shading.md (§3)
//! The play preview's lighting (stitch-lighting; replaces the D1
//! full-bright fill of [`super::preview`] unless `D2RS_FULLBRIGHT=1`).
//!
//! Per drawn frame ([`PreviewLight::refresh`]): the 48 × 48 light map
//! around the local player (§1), the ambient fill from the act
//! environment the bridge steps per client update (§3, §9.2 r1), then
//! §6.4 over the client's kept light list (`ClientWorld::lights`, §6.3:
//! records the model's unit code created, §8), contributing by kind
//! (r5); then the cel light value of a unit (§11 r1) and of a tile
//! ([`tile_chain`]) is read from it, and shaded through the act's PL2
//! tables.
//!
//! `d2rs-own, unverified`:
//! - a tile's whole-tile shade is one flat value (the cell at its centre
//!   sub-tile); its blocks take the gradients of [`super::preview_blocks`];
//! - the ambient of a room is the scripted override (§10, client quest
//!   byte 1 from the last S→C 0x5E), else the level's `Levels.txt`
//!   ambient when it has a colour, else the environment's; the near rooms
//!   fill their rectangles (§3 r3);
//! - the blocks-light flags (§4) and the kind-2 caches (§7.4 r1) come
//!   from the client DRLG collision (`collision_at`, mask 0x22);
//! - the other units' lights sit at their sub-tile `<< 16` (the model
//!   holds no precise position); the local player's at its predicted
//!   16.16 position (§6.1);

use crate::bridge::world::ClientWorld;
use crate::bridge::{ClientUnit, UnitKey};
use crate::composite::ComponentRequest;
use crate::rules::draw_order::{Dt1Facts, TileKind};
use crate::rules::lighting::contribute;
use crate::rules::lighting::draws::MATERIAL_UNLIT;
use crate::rules::lighting::environment::{Ambient as EnvAmbient, Environment, PeriodTables};
use crate::rules::lighting::map::{Ambient, AmbientScene, LightMap, NearRoom};
use crate::rules::lighting::records::{
    LightKind, LightList, LightRooms, LightWorld, Owner, RoomId,
};
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
    /// The last refresh's failure ([`Self::error`]).
    error: Option<String>,
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

/// A light record of the frame (§6.1, §8): position in 1/8 sub-tile,
/// radius in sub-tiles, colour, the §8 kind and the owner's sub-tile
/// (the centre of a kind-2 cache, §7.4 r1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceLight {
    pub at: (i32, i32),
    pub radius: i32,
    pub rgb: (u8, u8, u8),
    pub kind: LightKind,
    pub owner: (i32, i32),
}

impl SourceLight {
    /// A light of a unit standing on sub-tile `s`: `(P >> 13) + 4` =
    /// `8·s + 4` (§6.1).
    pub fn at_subtile(s: (i32, i32), radius: i32, rgb: (u8, u8, u8), kind: LightKind) -> Self {
        SourceLight {
            at: (8 * s.0 + 4, 8 * s.1 + 4),
            radius,
            rgb,
            kind,
            owner: s,
        }
    }
}

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
    blocks: impl Fn(i32, i32) -> bool,
    lights: &[PointLight],
    shadow_first: bool,
) -> LightMap {
    let lights: Vec<SourceLight> = lights
        .iter()
        .enumerate()
        .map(|(n, &(s, r, rgb))| {
            let kind = if shadow_first && n == 0 {
                LightKind::Shadowed
            } else {
                LightKind::Plain
            };
            SourceLight::at_subtile(s, r, rgb, kind)
        })
        .collect();
    build_map_eighths(player, scene, blocks, &lights)
}

/// The light position of a unit at 16.16 position `(x16, y16)` (§6.1,
/// `0x006203B0`): `(P >> 13) + 4` on each axis, in 1/8 sub-tile.
pub fn light_pos_of(x16: u32, y16: u32) -> (i32, i32) {
    (
        crate::rules::lighting::records::unit_light_pos(x16 as i32),
        crate::rules::lighting::records::unit_light_pos(y16 as i32),
    )
}

/// The light map of a frame from its records (§2): ambient fill, the
/// blocks-light flags, then each record by its kind (§6.4 r5): kind 0
/// shadowed at `q` = 2 (§7.3), kind 2 cached (§7.4, the cache built from
/// `blocks` around the owner's sub-tile), kind 1 plain (§7.2).
pub fn build_map_eighths(
    player: (i32, i32),
    scene: &AmbientScene,
    blocks: impl Fn(i32, i32) -> bool,
    lights: &[SourceLight],
) -> LightMap {
    let mut map = LightMap::new(player);
    map.fill_ambient(Some(scene));
    map.fill_blocks(&blocks);
    let mut list = LightList::new();
    let mut owners = Vec::with_capacity(lights.len());
    for l in lights {
        let (red, green, blue) = l.rgb;
        if let Some(id) = list.create(None, l.at, l.kind, l.radius, 255, red, green, blue) {
            owners.push((id, l.owner));
        }
    }
    let colored = list.colored;
    for (id, rec) in list.iter() {
        match rec.kind {
            LightKind::Shadowed if QUALITY > 1 => contribute::shadowed(&mut map, rec, colored),
            LightKind::Cached => {
                let owner = owners
                    .iter()
                    .find_map(|&(i, o)| (i == id).then_some(o))
                    .expect("every record has its source");
                let mut rec = rec.clone();
                rec.cache = contribute::build_cache(&rec, owner, &blocks);
                rec.cache_valid = true;
                contribute::cached_contribution(&mut map, &rec, colored);
            }
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

/// The client world as a drawn frame's §6.4 pass reads it: owners through
/// the model's lookups (`ClientWorld` [`LightRooms`]), the local player at
/// its predicted 16.16 position, the blocks-light test over the client
/// DRLG collision (§4, mask 0x22).
///
/// `d2rs-own, unverified`: the model holds no 16.16 position of other
/// units, so their position is their sub-tile `<< 16` (the static-path
/// form of §6.1).
struct FrameOwners<'a> {
    world: &'a ClientWorld,
    local: Option<(UnitKey, (u32, u32))>,
    drlg: Option<&'a d2_sim::drlg::Drlg>,
}

impl FrameOwners<'_> {
    /// §4: the blocks-light flag of sub-tile `(x, y)`; with no client DRLG
    /// (or no player room) nothing blocks.
    fn blocks(&self, x: i32, y: i32) -> bool {
        self.drlg
            .is_some_and(|d| blocks_light(d.collision_at(x, y)))
    }
}

impl LightRooms for FrameOwners<'_> {
    fn owner_position(&self, owner: &Owner) -> Option<(i32, i32)> {
        let u = self.world.light_owner(owner)?;
        if let Some((key, (x, y))) = self.local {
            if key == u.key && !owner.client_only {
                return Some((x as i32, y as i32));
            }
        }
        let (x, y) = u.cell();
        Some((i32::from(x) << 16, i32::from(y) << 16))
    }

    fn owner_subtile(&self, owner: &Owner) -> Option<(i32, i32)> {
        self.world.owner_subtile(owner)
    }

    fn owner_room(&self, owner: &Owner) -> Option<RoomId> {
        self.world.owner_room(owner)
    }

    fn cell_room(&self, room: RoomId, x: i32, y: i32) -> Option<RoomId> {
        self.world.cell_room(room, x, y)
    }
}

impl LightWorld for FrameOwners<'_> {
    fn is_local_player(&self, owner: &Owner) -> bool {
        !owner.client_only
            && self
                .world
                .local_player
                .is_some_and(|k| u32::from(k.unit_type) == owner.unit_type && k.guid == owner.guid)
    }

    fn owner_blocks(&self, _: &Owner, x: i32, y: i32) -> bool {
        self.blocks(x, y)
    }
}

impl PreviewLight {
    /// The ambient of a room of `level` (§3.1): the scripted override
    /// (§10) when it has a colour, else the level's own, else the act's
    /// `env`.
    fn room_ambient(
        &self,
        world: &ClientWorld,
        level: u32,
        env: Ambient,
    ) -> Result<Ambient, String> {
        // §10 r1: client quest byte 1, read only in level 8 while the Den
        // flag is 0; no 0x5E yet there is fatal 0x60 in 1.14d.
        let quest = if level == crate::rules::lighting::overrides::LEVEL_DEN_OF_EVIL
            && !world.overrides.den_flag
        {
            world
                .client_quest_byte(1)
                .ok_or("fatal 0x60: client quest byte 1 read before any S→C 0x5E")?
        } else {
            0
        };
        let o = world.overrides.ambient(level, quest);
        if o.r != 0 || o.g != 0 || o.b != 0 {
            return Ok(Ambient {
                i: o.i,
                r: o.r,
                g: o.g,
                b: o.b,
            });
        }
        Ok(self
            .sources
            .as_ref()
            .and_then(|s| level_ambient(&s.levels, level))
            .unwrap_or(env))
    }

    /// The ambient input of §3: the player room's ambient and the near
    /// list (its adjacency array, `rooms.md` §6), each with its own level
    /// ambient and sub-tile rectangle.
    fn scene(&self, world: &ClientWorld, env: Ambient, level: u32) -> Result<AmbientScene, String> {
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
                    ambient: self.room_ambient(world, u32::from(r.level), env)?,
                    is_player_room: id == own.room,
                });
            }
        }
        Ok(AmbientScene {
            player_ambient: self.room_ambient(world, level, env)?,
            near,
        })
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

    /// The frame's light (once per drawn frame, before the build;
    /// `lighting.md` §2): the ambient fill (§3), the blocks-light flags
    /// (§4), then §6.4 over the client's kept list `lights` (§6.3): each
    /// record's position from its owner, its radius walk (r2), dying
    /// records removed (r4), the contribution by kind (r5), kind-2 caches
    /// built once and kept until a radius change or a new room drops them
    /// (§6.4 r2, last paragraph). `local_at` is the predicted 16.16
    /// position of the local player.
    pub fn refresh(
        &mut self,
        world: &ClientWorld,
        lights: &mut LightList,
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
        let local_at = local_at.filter(|(key, _)| *key == player.key);
        let at = match local_at {
            Some((_, (x, y))) => subtile_of(x, y),
            None => player
                .position
                .map_or((0, 0), |(x, y)| (i32::from(x), i32::from(y))),
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
        let scene = match self.scene(world, ambient, level) {
            Ok(scene) => scene,
            Err(e) => {
                self.error = Some(e);
                return;
            }
        };
        let drlg = world.drlg.as_ref().filter(|_| world.local_room().is_some());
        let owners = FrameOwners {
            world,
            local: local_at,
            drlg: drlg.map(|d| &d.drlg),
        };
        let mut map = LightMap::new(at);
        map.fill_ambient(Some(&scene));
        map.fill_blocks(&|x, y| owners.blocks(x, y));
        // A fatal case of 1.14d (§6.4, §7.4) stops the list walk; the map
        // keeps what the records before it gave.
        self.error = lights
            .frame(&mut map, QUALITY, &owners)
            .err()
            .map(|e| e.to_string());
        self.frame = Some(FrameLight {
            tables: *tables,
            map,
        });
    }

    /// The last refresh's failure (a 1.14d fatal case), if any.
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
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

    /// A drawn frame's light pass over the model's kept list, as
    /// `present.rs` runs it (`Bridge::light_frame`).
    fn refresh(
        l: &mut PreviewLight,
        w: &mut ClientWorld,
        at: Option<(UnitKey, (u32, u32))>,
        t: &ShadeTables,
    ) {
        let mut lights = std::mem::take(&mut w.lights);
        l.refresh(w, &mut lights, at, Some(t));
        w.lights = lights;
    }

    /// The local player as 0x59 leaves it: its light record (§8 player
    /// row) in the model's list.
    fn local_player(w: &mut ClientWorld, key: UnitKey, at: (u16, u16)) {
        let mut u = ClientUnit::new(key);
        u.position = Some(at);
        w.units.insert(key, u);
        w.local_player = Some(key);
        crate::bridge::msg::lighting::player_light(w, key);
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
        local_player(&mut w, key, (4000, 4000));
        let t = tables();
        let mut light = PreviewLight::default();
        refresh(&mut light, &mut w, None, &t);
        let f = light.frame().expect("frame light");
        let at = f.map.read(8 * 4000, 8 * 4000).i;
        let far = f.map.read(8 * (4000 + 20), 8 * 4000).i;
        assert!(at > far, "lit {at} > outside the radius {far}");
        // The unit's light is the cell at its sub-tile.
        let unit = w.units.get(&key).unwrap();
        assert_eq!(light.look().light_subtile(unit).unwrap(), (4000, 4000));
        // Stat 89 widens the radius.
        let mut w2 = w.clone();
        // The stat write runs the player list's callback (`0x004609F0`),
        // as S→C 0x20 does in the model.
        w2.units.get_mut(&key).unwrap().stats.insert(89, 4);
        crate::bridge::msg::lighting::player_light_stat(&mut w2, key, 89, 0, 4);
        let mut wide = PreviewLight::default();
        refresh(&mut wide, &mut w2, None, &t);
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
        local_player(&mut w, key, (100, 100));
        let t = tables();
        let mut map = |x16: u32| {
            let mut l = PreviewLight::default();
            refresh(&mut l, &mut w, Some((key, (x16, 100 << 16))), &t);
            l.frame().unwrap().map.clone()
        };
        let whole = map(100 << 16);
        let frac = map((100 << 16) + 0xC000);
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
        local_player(&mut w, key, (4000, 4000));
        let t = tables();
        let far = (8 * 4000 + 8 * 40, 8 * 4000);
        let mut light = PreviewLight::default();
        refresh(&mut light, &mut w, None, &t);
        assert_eq!(light.frame().unwrap().map.read(far.0, far.1).i, 128);
        // Noon (90 degrees, index 2): §9.3 r4 gives 255.
        let periods = PeriodTables::builtin().unwrap();
        let mut env = Environment::new(&periods, 0);
        env.ticks = 90 * 128;
        // As the bridge's client update leaves it (§9.2 r1): the map
        // build reads the record, it no longer steps it.
        env.update(&periods, 0);
        w.environment = Some(env);
        refresh(&mut light, &mut w, None, &t);
        assert_eq!(light.frame().unwrap().map.read(far.0, far.1).i, 255);
    }

    // Covers: specs/render/lighting.md §6.4 r5, §7.3, §7.4
    #[test]
    fn each_light_contributes_by_its_kind() {
        let scene = AmbientScene {
            player_ambient: dark(),
            near: Vec::new(),
        };
        // A wall column at sub-tile x = 103 between the light (100, 100)
        // and the cell (106, 100).
        let wall = |x: i32, _: i32| x == 103;
        let at = |kind, blocks: &dyn Fn(i32, i32) -> bool| {
            let l = SourceLight::at_subtile((100, 100), 8, (255, 255, 255), kind);
            build_map_eighths((100, 100), &scene, blocks, &[l]).read(8 * 106, 8 * 100)
        };
        for kind in [LightKind::Shadowed, LightKind::Cached] {
            let (open, shut) = (at(kind, &|_, _| false), at(kind, &wall));
            assert!(shut.i < open.i, "{kind:?}: {} < {}", shut.i, open.i);
        }
        // Kind 1 (missiles, other players) shines through.
        assert_eq!(
            at(LightKind::Plain, &wall),
            at(LightKind::Plain, &|_, _| false)
        );
        // Without walls every kind lights alike (§7.3 / §7.4 with S = 0).
        let open = at(LightKind::Plain, &|_, _| false);
        assert_eq!(at(LightKind::Shadowed, &|_, _| false).i, open.i);
        assert_eq!(at(LightKind::Cached, &|_, _| false).i, open.i);
    }

    // Covers: specs/render/lighting.md §9.2 r1
    #[test]
    fn drawn_frames_do_not_step_the_environment() {
        let mut w = ClientWorld::default();
        let key = UnitKey::new(PLAYER, 1);
        local_player(&mut w, key, (4000, 4000));
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
            refresh(&mut light, &mut w, None, &t);
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
        local_player(&mut w, key, (4000, 4000));
        let t = tables();
        let far = (8 * 4000 + 8 * 40, 8 * 4000);
        let mut plain = PreviewLight::default();
        refresh(&mut plain, &mut w, None, &t);
        // Level 0 (no room): the row's own ambient needs a colour.
        let rows = super::super::light_sources::LightRows {
            levels: vec![(99, 255, 255, 255)],
        };
        let mut lit = PreviewLight {
            sources: Some(std::sync::Arc::new(rows)),
            ..PreviewLight::default()
        };
        refresh(&mut lit, &mut w, None, &t);
        assert_eq!(lit.frame().unwrap().map.read(far.0, far.1).i, 99);
        assert_ne!(plain.frame().unwrap().map.read(far.0, far.1).i, 99);
    }

    // Covers: specs/render/lighting.md §8
    #[test]
    fn a_monster_light_lights_its_surroundings() {
        use crate::bridge::world::{MonsterClass, MONSTER};
        let mut w = ClientWorld::default();
        let key = UnitKey::new(PLAYER, 1);
        local_player(&mut w, key, (4000, 4000));
        let mk = UnitKey::new(MONSTER, 9);
        let mut m = ClientUnit::new(mk);
        m.class = 0;
        m.mode = 1;
        m.position = Some((4000, 4018));
        w.units.insert(mk, m);
        let t = tables();
        let probe = (8 * 4000, 8 * 4018);
        let mut plain = PreviewLight::default();
        refresh(&mut plain, &mut w, None, &t);
        let dark = plain.frame().unwrap().map.read(probe.0, probe.1).i;
        // The monster init's light (0xAC, `monstats2` `Light` 6).
        let class = MonsterClass {
            light: 6,
            light_rgb: (255, 255, 255),
            ..MonsterClass::default()
        };
        crate::bridge::msg::lighting::monster_light(&mut w, mk, &class);
        let mut lit = PreviewLight::default();
        refresh(&mut lit, &mut w, None, &t);
        assert!(lit.frame().unwrap().map.read(probe.0, probe.1).i > dark);
    }

    // Covers: specs/render/lighting.md §6.4 r2, §6.4 r4, §7.4
    #[test]
    fn the_kept_list_walks_radii_keeps_caches_and_drops_the_dead() {
        use crate::bridge::drlg::DrlgRoomId;
        use crate::bridge::world::OBJECT;
        use crate::rules::lighting::records::LightKind;
        let mut w = ClientWorld::default();
        let key = UnitKey::new(PLAYER, 1);
        local_player(&mut w, key, (4000, 4000));
        // A torch (kind 2) in room 1, `Lit` 12: radius 6.
        let ok = UnitKey::new(OBJECT, 5);
        let mut o = ClientUnit::new(ok);
        o.position = Some((4004, 4000));
        w.units.insert(ok, o);
        w.room_units.place(ok, Some(DrlgRoomId(1)));
        crate::bridge::msg::lighting::object_light(&mut w, ok, 12, (255, 255, 255));
        let torch = crate::bridge::msg::lighting::unit_light(&w, ok).unwrap();
        let t = tables();
        let mut light = PreviewLight::default();
        refresh(&mut light, &mut w, None, &t);
        assert_eq!(light.error(), None);
        let rec = w.lights.get(torch).unwrap();
        assert!(rec.cache_valid && rec.radius == 48);
        // §7.4 r1: built once and kept; a mark in the cache survives the
        // next frame (no rebuild).
        w.lights.get_mut(torch).unwrap().cache[0] = 12_345;
        refresh(&mut light, &mut w, None, &t);
        assert_eq!(w.lights.get(torch).unwrap().cache[0], 12_345);
        // A mode with `Lit` 16 sets the target 8 (§8 object row): the
        // radius walks there by 8 per drawn frame (§6.4 r2) and each step
        // drops the cache (rebuilt for the frame).
        crate::bridge::msg::lighting::object_light(&mut w, ok, 16, (255, 255, 255));
        refresh(&mut light, &mut w, None, &t);
        let rec = w.lights.get(torch).unwrap();
        assert_eq!((rec.radius, rec.target), (56, 64));
        assert!(rec.cache_valid && rec.cache[0] != 12_345);
        refresh(&mut light, &mut w, None, &t);
        refresh(&mut light, &mut w, None, &t);
        assert_eq!(w.lights.get(torch).unwrap().radius, 64);
        // A dying light (§6.2 r6) shrinks by 8 per frame and is removed
        // once below 1 (§6.4 r4).
        let dying = w
            .lights
            .create(
                None,
                (8 * 4002 + 4, 8 * 4000 + 4),
                LightKind::Plain,
                2,
                255,
                1,
                2,
                3,
            )
            .unwrap();
        w.lights.die(dying).unwrap();
        refresh(&mut light, &mut w, None, &t);
        assert_eq!(w.lights.get(dying).map(|r| r.radius), Some(8));
        refresh(&mut light, &mut w, None, &t);
        assert!(w.lights.get(dying).is_none());
    }

    // Covers: specs/render/lighting.md §10 r1
    #[test]
    fn the_den_glow_reads_the_clients_quest_byte() {
        use crate::bridge::drlg::DrlgRoomId;
        use crate::bridge::world::ActiveRoom;
        let mut w = ClientWorld::default();
        let key = UnitKey::new(PLAYER, 1);
        local_player(&mut w, key, (4000, 4000));
        // The local player in a room of level 8, Den flag 0, counter −1.
        w.active_rooms = Some(vec![ActiveRoom {
            x0: 3900,
            y0: 3900,
            w: 200,
            h: 200,
            level: 8,
            room: DrlgRoomId(1),
        }]);
        w.room_units.place(key, Some(DrlgRoomId(1)));
        let t = tables();
        let far = (8 * 4000 + 8 * 40, 8 * 4000);
        let mut light = PreviewLight::default();
        // No 0x5E yet: 1.14d is fatal 0x60; no light map.
        refresh(&mut light, &mut w, None, &t);
        assert!(light.frame().is_none());
        assert!(light.error().unwrap().contains("0x60"));
        // Quest byte 1 = 0: no glow, the act ambient.
        let mut a = [0u8; 37];
        w.quest_availability = Some(a);
        refresh(&mut light, &mut w, None, &t);
        let plain = light.frame().unwrap().map.read(far.0, far.1);
        assert_ne!((plain.r, plain.g, plain.b), (255, 64, 48));
        // Set: the red glow, I = 80 while the Den counter is −1.
        a[1] = 1;
        w.quest_availability = Some(a);
        refresh(&mut light, &mut w, None, &t);
        let glow = light.frame().unwrap().map.read(far.0, far.1);
        assert_eq!((glow.i, glow.r, glow.g, glow.b), (80, 255, 64, 48));
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
        refresh(&mut light, &mut w, None, &t);
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
        local_player(&mut w, key, (900, 900));
        let t = tables();
        let mut feed = ModelFeed::<NoFeed>::default().with_preview(Preview::default());
        assert!(feed.light(&w).unwrap().is_none(), "before the first frame");
        refresh(&mut feed.preview.as_mut().unwrap().light, &mut w, None, &t);
        let l = feed.light(&w).unwrap().expect("lit");
        assert!(l.light.map.read(8 * 900, 8 * 900).i > l.light.map.read(8 * 940, 8 * 900).i);
        feed.preview.as_mut().unwrap().light.fullbright = true;
        refresh(&mut feed.preview.as_mut().unwrap().light, &mut w, None, &t);
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
