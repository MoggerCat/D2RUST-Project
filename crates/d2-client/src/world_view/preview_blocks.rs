// Spec: specs/render/lighting.md (§11 r2–r4), specs/render/shading.md (§4), specs/render/blend-modes.md (§6)
//! Per-block light of the play preview's tiles (q-lighting-detail): the
//! wall / floor / roof gradients of `shading.md` §4 from the frame's light
//! map, through the CPU reference rules ([`crate::rules::lighting::draws`],
//! [`crate::rules::shading`]) that the GPU path is checked against.
//!
//! `d2rs-own, unverified`:
//! - a roof takes the floor grid path (§11 r4: "the same grid") with the
//!   roof's wall alpha blend; `shading.md` §4 names walls *and roofs* for
//!   the corner path, so which one a roof block takes is open (PROVISIONAL);
//! - a tile whose light cannot be read (wall direction 0 or past 9, a grid
//!   index out of range) keeps its flat tile shade.

use crate::rules::draw_order::{Dt1Facts, TileKind};
use crate::rules::lighting::draws::{
    floor_light_grid, roof_light_grid, wall_block_shades, wall_light_words,
};
use crate::rules::lighting::map::LightCell;
use crate::rules::lighting::view::FrameLight;
use crate::rules::shading::{floor_block_chain, floor_block_light};
use crate::rules::{BlockRect, BlockShade};
use crate::scene::BlendOp;

/// Sub-tiles per tile edge.
const SUBTILES: i32 = 5;

/// The per-block shades of the tile at absolute tile `cell`; empty when
/// the tile keeps its flat shade (shadow tiles, unreadable light).
/// `blend` is the tile's own blend ([`super::preview::tile_ops`]);
/// `fade` the record's alpha byte and fade state (+0x24 as the frame's
/// §8 filing left it: bit 0 reads the faded wall points, §11 r2).
#[allow(clippy::too_many_arguments)]
pub fn block_shades(
    light: &FrameLight,
    environment: LightCell,
    kind: TileKind,
    dt1: &Dt1Facts,
    cell: (i32, i32),
    fade: (u8, u8),
    blend: BlendOp,
    blocks: &[BlockRect],
    grids: &[(u8, u8)],
) -> Vec<BlockShade> {
    let (alpha, fade_state) = fade;
    let origin = (SUBTILES * cell.0, SUBTILES * cell.1);
    let grid_shades = |cells: [u8; 64]| -> Vec<BlockShade> {
        let mut out = Vec::with_capacity(blocks.len());
        for (block, &(gx, gy)) in blocks.iter().zip(grids) {
            let Ok(l) = floor_block_light(&cells, gx, gy, false) else {
                return Vec::new();
            };
            out.push(BlockShade {
                block: *block,
                shade: floor_block_chain(&light.tables, l, 0, 0),
                blend,
            });
        }
        out
    };
    match kind {
        TileKind::ShadowTile => Vec::new(),
        TileKind::Floor { .. } => grid_shades(
            floor_light_grid(&light.map, origin, u32::from(dt1.material)).light_values(),
        ),
        TileKind::Roof { .. } => grid_shades(
            roof_light_grid(
                &light.map,
                (8 * origin.0, 8 * origin.1),
                dt1.roof_height as u32,
                environment,
            )
            .light_values(),
        ),
        TileKind::Wall | TileKind::LowerWall => {
            let Ok(words) = wall_light_words(
                &light.map,
                origin,
                dt1.light_direction,
                u32::from(fade_state),
            ) else {
                return Vec::new();
            };
            wall_block_shades(&light.tables, &words, blocks, alpha, false).unwrap_or_default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::lighting::map::Ambient;
    use crate::rules::shading::ShadeTables;
    use crate::scene::{MapTable, ShadeChain};
    use crate::world_view::preview_light::{build_map, level_ambient};

    fn frame(lights: &[crate::world_view::preview_light::PointLight]) -> FrameLight {
        let pl2 =
            d2_formats::palette::Pl2::parse(&super::super::tile_assets::tests::pl2()).unwrap();
        let dark = Ambient {
            i: 40,
            r: 40,
            g: 40,
            b: 40,
        };
        FrameLight {
            tables: ShadeTables::push(&mut MapTable::new(), &pl2),
            map: build_map((100, 100), dark, lights),
        }
    }

    fn rect(x: i32) -> BlockRect {
        BlockRect {
            x,
            y: 0,
            width: 32,
            height: 32,
        }
    }

    fn env() -> LightCell {
        LightCell {
            blocks: 0,
            i: 77,
            r: 1,
            g: 2,
            b: 3,
        }
    }

    fn floor() -> TileKind {
        TileKind::Floor { layer: 1 }
    }

    // Covers: specs/render/lighting.md §11 r3
    #[test]
    fn a_floor_block_takes_the_gradient_corners_of_the_spec() {
        // A light centred in the tile's grid makes the neighbours differ.
        let f = frame(&[((102, 102), 6, (255, 255, 255))]);
        let cell = (20, 20); // origin sub-tile (100, 100)
        let e = |n: i32| i32::from(f.map.read(8 * (99 + n % 8), 8 * (99 + n / 8)).i);
        let g = 0;
        let delta = (e(g + 10) - e(g + 9)).abs()
            + (e(g + 17) - e(g + 9)).abs()
            + (e(g + 18) - e(g + 10)).abs();
        assert!(delta >= 10, "the light must make a gradient: {delta}");
        let want = [
            ((e(8) + e(9) + e(16) + e(17)) >> 2) as u8,
            ((e(1) + e(2) + e(9) + e(10)) >> 2) as u8,
            ((e(10) + e(11) + e(17) + e(19)) >> 2) as u8,
            ((e(17) + e(18) + e(25) + e(26)) >> 2) as u8,
        ];
        let out = block_shades(
            &f,
            env(),
            floor(),
            &Dt1Facts::default(),
            cell,
            (0xFF, 0),
            BlendOp::Opaque,
            &[rect(0)],
            &[(0, 0)],
        );
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].shade.gradient().expect("gradient").corners, want);
    }

    // Covers: specs/render/shading.md §4
    #[test]
    fn a_flat_floor_block_uses_one_light_map() {
        let f = frame(&[]);
        let out = block_shades(
            &f,
            env(),
            floor(),
            &Dt1Facts::default(),
            (20, 20),
            (0xFF, 0),
            BlendOp::Opaque,
            &[rect(0)],
            &[(0, 0)],
        );
        // Ambient 40: every cell 40, flat, light map 40 >> 3 = 5.
        assert!(out[0].shade.gradient().is_none());
        assert_eq!(
            out[0].shade,
            ShadeChain::new(&[f.tables.light_map(5)]).unwrap()
        );
    }

    // Covers: specs/render/lighting.md §11 r3
    #[test]
    fn an_unlit_floor_material_is_light_map_31() {
        let f = frame(&[((102, 102), 6, (255, 255, 255))]);
        let dt1 = Dt1Facts {
            material: 0x100,
            ..Dt1Facts::default()
        };
        let out = block_shades(
            &f,
            env(),
            floor(),
            &dt1,
            (20, 20),
            (0xFF, 0),
            BlendOp::Opaque,
            &[rect(0)],
            &[(0, 0)],
        );
        assert_eq!(
            out[0].shade,
            ShadeChain::new(&[f.tables.light_map(31)]).unwrap()
        );
    }

    // Covers: specs/render/lighting.md §11 r4
    #[test]
    fn a_high_roof_takes_the_environment_light() {
        let f = frame(&[((102, 102), 6, (255, 255, 255))]);
        let dt1 = Dt1Facts {
            roof_height: 3,
            ..Dt1Facts::default()
        };
        let out = block_shades(
            &f,
            env(),
            TileKind::Roof { pass: 1 },
            &dt1,
            (20, 20),
            (0xFF, 0),
            BlendOp::Opaque,
            &[rect(0)],
            &[(0, 0)],
        );
        // Environment I = 77 in every cell: flat, light map 77 >> 3 = 9.
        assert_eq!(
            out[0].shade,
            ShadeChain::new(&[f.tables.light_map(9)]).unwrap()
        );
    }

    // Covers: specs/render/lighting.md §11 r2
    #[test]
    fn a_wall_block_takes_its_column_corners_from_the_points() {
        let f = frame(&[((100, 100), 8, (255, 255, 255))]);
        let cell = (20, 20);
        for direction in 1..=9u32 {
            let dt1 = Dt1Facts {
                light_direction: direction,
                ..Dt1Facts::default()
            };
            let words = wall_light_words(&f.map, (100, 100), direction, 0).unwrap();
            let low = |w: u32| w as u8;
            for col in 0..2usize {
                let (a, b) = (low(words[col]), low(words[col + 1]));
                let out = block_shades(
                    &f,
                    env(),
                    TileKind::Wall,
                    &dt1,
                    cell,
                    (0xFF, 0),
                    BlendOp::Opaque,
                    &[rect(32 * col as i32)],
                    &[(0, 0)],
                );
                let want = crate::rules::shading::wall_block_light([a, b, b, a], false);
                let chain = f
                    .tables
                    .block_chain(want, crate::scene::GradientKind::Wall, 0, 0);
                assert_eq!(out[0].shade, chain, "direction {direction} column {col}");
            }
        }
    }

    // Covers: specs/render/lighting.md §11 r2
    #[test]
    fn a_wall_reads_the_points_of_its_light_direction_not_its_orientation() {
        // Open question 8 (measured on every 1.14d DT1 header): the light
        // direction is fixed by orientation, and differs from it for
        // orientations 0, 4–19.
        let pairs = [
            (0, 3),
            (1, 1),
            (2, 2),
            (3, 3),
            (4, 3),
            (5, 1),
            (6, 2),
            (7, 4),
            (8, 1),
            (9, 2),
            (10, 1),
            (11, 2),
            (12, 3),
            (13, 3),
            (14, 3),
            (16, 6),
            (17, 7),
            (18, 8),
            (19, 9),
        ];
        let f = frame(&[((100, 100), 8, (255, 255, 255))]);
        for (orientation, direction) in pairs {
            let dt1 = Dt1Facts {
                orientation,
                light_direction: direction,
                ..Dt1Facts::default()
            };
            let kind = if orientation >= 16 {
                TileKind::LowerWall
            } else {
                TileKind::Wall
            };
            let words = wall_light_words(&f.map, (100, 100), direction, 0).unwrap();
            let (a, b) = (words[0] as u8, words[1] as u8);
            let out = block_shades(
                &f,
                env(),
                kind,
                &dt1,
                (20, 20),
                (0xFF, 0),
                BlendOp::Opaque,
                &[rect(0)],
                &[(0, 0)],
            );
            let want = crate::rules::shading::wall_block_light([a, b, b, a], false);
            let chain = f
                .tables
                .block_chain(want, crate::scene::GradientKind::Wall, 0, 0);
            assert_eq!(out.len(), 1, "orientation {orientation}");
            assert_eq!(out[0].shade, chain, "orientation {orientation}");
        }
    }

    // Covers: specs/render/lighting.md §11 r2
    #[test]
    fn a_fading_wall_reads_the_faded_points() {
        let f = frame(&[((100, 100), 8, (255, 255, 255))]);
        let shade = |direction: u32, state: u8| {
            let dt1 = Dt1Facts {
                light_direction: direction,
                ..Dt1Facts::default()
            };
            block_shades(
                &f,
                env(),
                TileKind::Wall,
                &dt1,
                (20, 20),
                (0xFF, state),
                BlendOp::Opaque,
                &[rect(0)],
                &[(0, 0)],
            )[0]
            .shade
        };
        let mut differs = false;
        for direction in 1..=9u32 {
            // Fade state bit 0 (set with bit 1 by the §8 target update)
            // selects the faded table; bit 1 alone does not.
            for (state, fade) in [(0, 0), (2, 0), (3, 1), (1, 1)] {
                let words = wall_light_words(&f.map, (100, 100), direction, fade).unwrap();
                let (a, b) = (words[0] as u8, words[1] as u8);
                let want = crate::rules::shading::wall_block_light([a, b, b, a], false);
                let chain = f
                    .tables
                    .block_chain(want, crate::scene::GradientKind::Wall, 0, 0);
                assert_eq!(shade(direction, state), chain, "{direction} {state}");
            }
            differs |= shade(direction, 3) != shade(direction, 0);
        }
        assert!(differs, "the faded table must change some wall's light");
    }

    // Covers: specs/render/lighting.md §11 r2
    #[test]
    fn unreadable_light_keeps_the_flat_tile_shade() {
        let f = frame(&[]);
        let dt1 = Dt1Facts::default(); // light direction 0
        let out = block_shades(
            &f,
            env(),
            TileKind::Wall,
            &dt1,
            (20, 20),
            (0xFF, 0),
            BlendOp::Opaque,
            &[rect(0)],
            &[(0, 0)],
        );
        assert!(out.is_empty());
        assert!(block_shades(
            &f,
            env(),
            TileKind::ShadowTile,
            &dt1,
            (20, 20),
            (0xFF, 0),
            BlendOp::Opaque,
            &[rect(0)],
            &[(0, 0)]
        )
        .is_empty());
    }

    // Covers: specs/render/lighting.md §4, §7.3
    #[test]
    fn a_blocks_light_tile_between_the_light_and_a_floor_block_darkens_it() {
        use crate::rules::lighting::map::AmbientScene;
        use crate::world_view::preview_light::build_map_blocked;
        let dark = Ambient {
            i: 40,
            r: 40,
            g: 40,
            b: 40,
        };
        let scene = AmbientScene {
            player_ambient: dark,
            near: Vec::new(),
        };
        let light = [((100, 100), 8, (255, 255, 255))];
        let pl2 =
            d2_formats::palette::Pl2::parse(&super::super::tile_assets::tests::pl2()).unwrap();
        let tables = ShadeTables::push(&mut MapTable::new(), &pl2);
        // A wall column at sub-tile x = 103 between the light (100) and the
        // floor tile whose origin sub-tile is (105, 100).
        let open = build_map_blocked((100, 100), &scene, |_, _| false, &light, true);
        let shut = build_map_blocked((100, 100), &scene, |x, _| x == 103, &light, true);
        let at = |m: &crate::rules::lighting::map::LightMap| m.read(8 * 106, 8 * 101).i;
        assert!(at(&shut) < at(&open), "{} < {}", at(&shut), at(&open));
        let shades = |map| {
            block_shades(
                &FrameLight { tables, map },
                env(),
                floor(),
                &Dt1Facts::default(),
                (21, 20),
                (0xFF, 0),
                BlendOp::Opaque,
                &[rect(0)],
                &[(0, 0)],
            )
        };
        assert_ne!(shades(open)[0].shade, shades(shut)[0].shade);
    }

    // Covers: specs/render/lighting.md §3.1 r2
    #[test]
    fn the_level_ambient_needs_a_colour() {
        let rows = [(0, 0, 0, 0), (30, 255, 255, 255), (8, 0, 0, 1)];
        assert_eq!(level_ambient(&rows, 0), None, "no colour: the environment");
        assert_eq!(
            level_ambient(&rows, 1),
            Some(Ambient {
                i: 30,
                r: 255,
                g: 255,
                b: 255
            })
        );
        assert_eq!(level_ambient(&rows, 2).map(|a| a.i), Some(8));
        assert_eq!(level_ambient(&rows, 9), None, "past the table");
    }
}
