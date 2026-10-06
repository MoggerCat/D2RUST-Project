// Spec: specs/render/map-preview.md (Cells, Screen position, What is drawn, Draw order)
//! Turns a DS1 plus a tile library into an ordered list of draw items.

use std::collections::BTreeMap;

use d2_formats::ds1::Ds1;

use super::tiles::{TileKey, TileLibrary};

/// Tile cell size in pixels.
pub const CELL_WIDTH: i32 = 160;
pub const CELL_HEIGHT: i32 = 80;

/// Which DS1 layer an item came from (for debugging and probes).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    Floor(usize),
    Wall(usize),
    Sprite,
}

/// One tile image drawn at a screen position (top-left, y down).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DrawItem {
    pub image: usize,
    pub x: i32,
    pub y: i32,
    pub key: TileKey,
    pub cell: (i32, i32),
    pub source: Source,
}

/// Screen-space bounding box `[x0, x1) × [y0, y1)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bounds {
    pub x0: i32,
    pub y0: i32,
    pub x1: i32,
    pub y1: i32,
}

#[derive(Debug, Default)]
pub struct Layout {
    /// Back to front.
    pub items: Vec<DrawItem>,
    /// Keys referenced by the map but absent from the library, with counts.
    pub missing: BTreeMap<TileKey, usize>,
    pub bounds: Option<Bounds>,
}

/// Screen origin of cell `(x, y)` (spec §Screen position).
pub fn cell_origin(x: i32, y: i32) -> (i32, i32) {
    ((x - y) * CELL_WIDTH / 2, (x + y) * CELL_HEIGHT / 2)
}

/// Cell flag: the tile is not drawn (boundary/collision markers).
pub const HIDDEN: u32 = 0x8000_0000;

/// `(main, sub)` of a drawn cell: `None` if empty or hidden (spec §Cells).
pub fn cell_indices(cell: u32) -> Option<(u32, u32)> {
    if cell & 0xFF == 0 || cell & HIDDEN != 0 {
        return None;
    }
    Some(((cell >> 20) & 0x3F, (cell >> 8) & 0xFF))
}

const ROOF: u32 = 15;

fn is_lower_wall(orientation: u32) -> bool {
    (16..=19).contains(&orientation)
}

fn is_skipped(orientation: u32) -> bool {
    matches!(orientation, 0 | 10 | 11 | 13)
}

/// A wall-layer piece before sorting.
struct Piece {
    depth: (i32, i32, usize, u8),
    item: DrawItem,
}

/// Builds the draw list. `wall_base` is the spec's `WALL_BASE`.
pub fn build(ds1: &Ds1, lib: &TileLibrary, wall_base: i32) -> Layout {
    let mut layout = Layout::default();
    let width = ds1.width as usize;
    let place =
        |layout: &mut Layout, key: TileKey, x: i32, y: i32, source: Source| -> Option<DrawItem> {
            match lib.lookup(key) {
                None => {
                    *layout.missing.entry(key).or_insert(0) += 1;
                    None
                }
                Some(None) => None,
                Some(Some(image)) => {
                    let img = &lib.images[image];
                    let (sx, sy) = cell_origin(x, y);
                    let dy = match key.orientation {
                        0 => 0,
                        ROOF => wall_base - img.roof_height,
                        _ => wall_base,
                    };
                    Some(DrawItem {
                        image,
                        x: sx + img.x0,
                        y: sy + img.y0 + dy,
                        key,
                        cell: (x, y),
                        source,
                    })
                }
            }
        };

    // 1. Floors, layer by layer, row-major.
    for (layer, floor) in ds1.floors.iter().enumerate() {
        for (i, &cell) in floor.iter().enumerate() {
            let Some((main, sub)) = cell_indices(cell) else {
                continue;
            };
            let (x, y) = ((i % width) as i32, (i / width) as i32);
            let key = TileKey {
                orientation: 0,
                main,
                sub,
            };
            if let Some(item) = place(&mut layout, key, x, y, Source::Floor(layer)) {
                layout.items.push(item);
            }
        }
    }

    // 2–4. Wall-layer pieces, grouped and depth-sorted.
    let (mut lower, mut walls, mut roofs) = (Vec::new(), Vec::new(), Vec::new());
    for (layer, (cells, orientations)) in ds1.walls.iter().zip(&ds1.orientations).enumerate() {
        for (i, (&cell, &orientation)) in cells.iter().zip(orientations).enumerate() {
            if is_skipped(orientation) {
                continue;
            }
            let Some((main, sub)) = cell_indices(cell) else {
                continue;
            };
            let (x, y) = ((i % width) as i32, (i / width) as i32);
            let mut parts = vec![(orientation, 0u8)];
            if orientation == 3 {
                parts.push((4, 1));
            }
            for (o, piece) in parts {
                let key = TileKey {
                    orientation: o,
                    main,
                    sub,
                };
                if let Some(item) = place(&mut layout, key, x, y, Source::Wall(layer)) {
                    let p = Piece {
                        depth: (x + y, x, layer, piece),
                        item,
                    };
                    if is_lower_wall(orientation) {
                        lower.push(p);
                    } else if orientation == ROOF {
                        roofs.push(p);
                    } else {
                        walls.push(p);
                    }
                }
            }
        }
    }
    for mut group in [lower, walls, roofs] {
        // Stable sort: equal depths keep cell order.
        group.sort_by_key(|p| p.depth);
        layout.items.extend(group.into_iter().map(|p| p.item));
    }

    layout.update_bounds(lib);
    layout
}

impl Layout {
    /// Items whose non-transparent pixels cover screen point `(x, y)`,
    /// topmost first.
    pub fn probe(&self, lib: &TileLibrary, x: i32, y: i32) -> Vec<(usize, DrawItem, u8)> {
        let mut hits: Vec<(usize, DrawItem, u8)> = self
            .items
            .iter()
            .enumerate()
            .filter_map(|(i, it)| {
                let img = &lib.images[it.image];
                let (px, py) = (x - it.x, y - it.y);
                if px < 0 || py < 0 || px >= img.width as i32 || py >= img.height as i32 {
                    return None;
                }
                let p = img.pixels[(py * img.width as i32 + px) as usize];
                (p != 0).then_some((i, *it, p))
            })
            .collect();
        hits.reverse();
        hits
    }

    /// Recomputes `bounds` from the items.
    pub fn update_bounds(&mut self, lib: &TileLibrary) {
        self.bounds = self
            .items
            .iter()
            .map(|it| {
                let img = &lib.images[it.image];
                Bounds {
                    x0: it.x,
                    y0: it.y,
                    x1: it.x + img.width as i32,
                    y1: it.y + img.height as i32,
                }
            })
            .reduce(|a, b| Bounds {
                x0: a.x0.min(b.x0),
                y0: a.y0.min(b.y0),
                x1: a.x1.max(b.x1),
                y1: a.y1.max(b.y1),
            });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::add_sprites;
    use d2_formats::dt1::{Dt1, Dt1Block, Dt1Tile};

    // Covers: specs/render/map-preview.md §screen-position
    #[test]
    fn origins() {
        assert_eq!(cell_origin(0, 0), (0, 0));
        assert_eq!(cell_origin(1, 0), (80, 40));
        assert_eq!(cell_origin(0, 1), (-80, 40));
    }

    // Covers: specs/render/map-preview.md §cells
    #[test]
    fn cell_fields() {
        assert_eq!(cell_indices(0x0050_00C2), Some((5, 0)));
        assert_eq!(cell_indices(0x0150_2A07), Some((0x15, 0x2A)));
        assert_eq!(cell_indices(0x0150_2A00), None, "prop1 = 0 is empty");
        // Real value from townN1.ds1 (river boundary): hidden, not drawn.
        assert_eq!(cell_indices(0x8050_0081), None, "hidden");
    }

    fn block(fill: u8) -> Dt1Block {
        Dt1Block {
            x: 0,
            y: 0,
            unknown1: 0,
            grid_x: 0,
            grid_y: 0,
            format: d2_formats::dt1::ISO_FORMAT,
            unknown2: 0,
            pixels: vec![fill; 32 * 15],
        }
    }

    fn tile(orientation: u32, sub: u32, roof_height: u16) -> Dt1Tile {
        Dt1Tile {
            light_direction: 0,
            roof_height,
            material_flags: 0,
            height: 0,
            width: 0,
            unknown_height: 0,
            orientation,
            main_index: 0,
            sub_index: sub,
            rarity: 0,
            unknown_color: 0,
            subtile_flags: [0; 25],
            unknown_58: 0,
            cache_index: 0,
            unknown_5c: 0,
            blocks: vec![block(orientation as u8 + 1)],
        }
    }

    /// 2×1 map: floors everywhere; wall layer 0 has a corner (3) at (1,0)
    /// and a roof at (0,0); wall layer 1 has a lower wall at (0,0).
    fn map() -> (Ds1, TileLibrary) {
        let cell = |sub: u32| 0x01 | (sub << 8);
        let ds1 = Ds1 {
            version: 18,
            width: 2,
            height: 1,
            act: 0,
            tag_type: 0,
            files: vec![],
            unknown_header: None,
            walls: vec![vec![cell(1), cell(1)], vec![cell(1), 0]],
            orientations: vec![vec![ROOF, 3], vec![16, 0]],
            floors: vec![vec![cell(1), cell(1)]],
            shadow: vec![0, 0],
            tags: None,
            objects: vec![],
            unknown_groups: None,
            groups: vec![],
            groups_truncated: false,
            paths: vec![],
            trailing: vec![],
        };
        let dt1 = Dt1 {
            version: 7,
            minor_version: 6,
            tiles: vec![
                tile(0, 1, 0),
                tile(3, 1, 0),
                tile(4, 1, 0),
                tile(ROOF, 1, 50),
                tile(16, 1, 0),
            ],
        };
        let mut lib = TileLibrary::new();
        lib.add_dt1(&dt1);
        (ds1, lib)
    }

    // Covers: specs/render/map-preview.md §draw-order r2, §draw-order r4, §what-is-drawn, §screen-position
    #[test]
    fn draw_order_and_positions() {
        let (ds1, lib) = map();
        let layout = build(&ds1, &lib, 80);
        let orient: Vec<u8> = layout
            .items
            .iter()
            .map(|it| lib.images[it.image].pixels[0] - 1)
            .collect();
        // floors, lower wall, corner (3 then 4), roof
        assert_eq!(orient, [0, 0, 16, 3, 4, 15]);
        let corner = &layout.items[3..5];
        assert_eq!((corner[0].x, corner[0].y), (80, 40 + 80));
        assert_eq!((corner[1].x, corner[1].y), (80, 40 + 80));
        let roof = layout.items[5];
        assert_eq!((roof.x, roof.y), (0, 80 - 50));
        assert!(layout.missing.is_empty());
        let b = layout.bounds.unwrap();
        assert_eq!((b.x0, b.y0, b.x1, b.y1), (0, 0, 112, 135));
    }

    // Covers: specs/render/map-preview.md §sprites-demo
    #[test]
    fn sprites_go_on_top() {
        use d2_formats::dc6::{Dc6, Dc6Frame, Dc6Header};
        use d2_formats::dcc::{Dcc, DccDirection, DccFrame};
        let (ds1, mut lib) = map();
        let mut layout = build(&ds1, &lib, 80);
        let before = layout.items.len();
        let dcc = Dcc {
            version: 6,
            frames_per_direction: 1,
            tag: 1,
            final_dc6_size: 0,
            directions: vec![DccDirection {
                outsize_coded: 0,
                compression_flags: 0,
                x_min: -2,
                y_min: -3,
                width: 2,
                height: 3,
                frames: vec![DccFrame {
                    variable0: 0,
                    width: 2,
                    height: 3,
                    x_offset: -2,
                    y_offset: -1,
                    coded_bytes: 0,
                    bottom_up: false,
                    optional_data: vec![],
                    x_min: -2,
                    y_min: -3,
                    pixels: vec![9; 6],
                }],
                pcd_leftover_bits: 0,
            }],
        };
        let frame = |w: u32| Dc6Frame {
            flip: 0,
            width: w,
            height: 1,
            offset_x: 0,
            offset_y: 0,
            unknown: 0,
            next_block: 0,
            pixels: vec![7; w as usize],
        };
        let dc6 = Dc6 {
            header: Dc6Header {
                version: 6,
                flags: 0,
                encoding: 0,
                termination: [0; 4],
                directions: 1,
                frames_per_direction: 2,
            },
            frames: vec![frame(4), frame(5)],
        };
        add_sprites(&mut layout, &mut lib, &ds1, &dcc, &dc6);
        let sprites = &layout.items[before..];
        assert_eq!(sprites.len(), 3);
        // Center cell (1, 0) anchor = origin (80, 40) + (80, 40).
        assert_eq!((sprites[0].x, sprites[0].y), (160 - 2, 80 - 3));
        // DC6 frames in a row at the bounds' top-left + 32.
        let b = layout.bounds.unwrap();
        assert_eq!((sprites[1].x, sprites[1].y), (b.x0 + 32, b.y0 + 32));
        assert_eq!(sprites[2].x, b.x0 + 32 + 4);
        assert!(sprites.iter().all(|s| s.source == Source::Sprite));
    }

    // Covers: specs/render/map-preview.md §what-is-drawn
    #[test]
    fn missing_tiles_are_counted() {
        let (mut ds1, lib) = map();
        ds1.floors[0][1] = 0x01 | (9 << 8);
        let layout = build(&ds1, &lib, 80);
        let key = TileKey {
            orientation: 0,
            main: 0,
            sub: 9,
        };
        assert_eq!(layout.missing.get(&key), Some(&1));
    }
}
