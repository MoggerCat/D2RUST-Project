// Spec: specs/render/map-preview.md (Tile files, Tile library and lookup, Tile images)
//! DT1 tiles assembled into images, and lookup by (orientation, main, sub).

use std::collections::BTreeMap;

use d2_formats::ds1::Ds1;
use d2_formats::dt1::{Dt1, Dt1Tile};

/// Identifies a tile within a map's tile set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TileKey {
    pub orientation: u32,
    pub main: u32,
    pub sub: u32,
}

/// A tile's blocks assembled into one 8-bit index image.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TileImage {
    /// Top-left of the image in tile coordinates.
    pub x0: i32,
    pub y0: i32,
    pub width: u32,
    pub height: u32,
    /// Palette indices, row-major, 0 = transparent.
    pub pixels: Vec<u8>,
    pub roof_height: i32,
}

/// Assembles a tile's blocks; `None` if it has no blocks.
pub fn assemble(tile: &Dt1Tile) -> Option<TileImage> {
    let rect = |b: &d2_formats::dt1::Dt1Block| {
        let (w, h) = b.size();
        let (x, y) = (i32::from(b.x), i32::from(b.y));
        (x, y, x + w as i32, y + h as i32)
    };
    let x0 = tile.blocks.iter().map(|b| rect(b).0).min()?;
    let y0 = tile.blocks.iter().map(|b| rect(b).1).min()?;
    let x1 = tile.blocks.iter().map(|b| rect(b).2).max()?;
    let y1 = tile.blocks.iter().map(|b| rect(b).3).max()?;
    let (width, height) = ((x1 - x0) as u32, (y1 - y0) as u32);
    let mut pixels = vec![0u8; width as usize * height as usize];
    for b in &tile.blocks {
        let (w, h) = b.size();
        let (bx, by) = (
            (i32::from(b.x) - x0) as usize,
            (i32::from(b.y) - y0) as usize,
        );
        for row in 0..h {
            for col in 0..w {
                let p = b.pixels[row * w + col];
                if p != 0 {
                    pixels[(by + row) * width as usize + bx + col] = p;
                }
            }
        }
    }
    Some(TileImage {
        x0,
        y0,
        width,
        height,
        pixels,
        roof_height: i32::from(tile.roof_height),
    })
}

/// Tile images for one map, looked up by key (first match in load order).
#[derive(Debug, Default)]
pub struct TileLibrary {
    pub images: Vec<TileImage>,
    /// Key → image index; `None` if the first tile with the key is empty.
    index: BTreeMap<TileKey, Option<usize>>,
}

impl TileLibrary {
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a DT1's tiles. Keys already present keep their first tile.
    pub fn add_dt1(&mut self, dt1: &Dt1) {
        for tile in &dt1.tiles {
            let key = TileKey {
                orientation: tile.orientation,
                main: tile.main_index,
                sub: tile.sub_index,
            };
            if self.index.contains_key(&key) {
                continue;
            }
            let image = assemble(tile).map(|img| {
                self.images.push(img);
                self.images.len() - 1
            });
            self.index.insert(key, image);
        }
    }

    /// `Some(Some(i))`: image `i`; `Some(None)`: the tile exists but is
    /// empty; `None`: no tile has this key.
    pub fn lookup(&self, key: TileKey) -> Option<Option<usize>> {
        self.index.get(&key).copied()
    }

    pub fn len(&self) -> usize {
        self.index.len()
    }

    pub fn is_empty(&self) -> bool {
        self.index.is_empty()
    }
}

/// Archive paths of the DT1 files a DS1 lists (spec §Tile files).
pub fn dt1_paths(ds1: &Ds1) -> Vec<String> {
    ds1.files
        .iter()
        .filter_map(|f| {
            let s = String::from_utf8_lossy(f);
            let at = s.to_ascii_lowercase().find(r"data\")?;
            let mut path = s[at..].to_string();
            if path.to_ascii_lowercase().ends_with(".tg1") {
                path.truncate(path.len() - 4);
                path.push_str(".dt1");
            }
            Some(path)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use d2_formats::dt1::Dt1Block;

    fn iso_block(x: i16, y: i16, fill: u8) -> Dt1Block {
        Dt1Block {
            x,
            y,
            unknown1: 0,
            grid_x: 0,
            grid_y: 0,
            format: d2_formats::dt1::ISO_FORMAT,
            unknown2: 0,
            pixels: vec![fill; 32 * 15],
        }
    }

    pub(crate) fn tile(orientation: u32, main: u32, sub: u32, blocks: Vec<Dt1Block>) -> Dt1Tile {
        Dt1Tile {
            light_direction: 0,
            roof_height: 0,
            material_flags: 0,
            height: 0,
            width: 0,
            unknown_height: 0,
            orientation,
            main_index: main,
            sub_index: sub,
            rarity: 0,
            unknown_color: 0,
            subtile_flags: [0; 25],
            unknown_58: 0,
            cache_index: 0,
            unknown_5c: 0,
            blocks,
        }
    }

    // Covers: specs/render/map-preview.md §tile-images
    #[test]
    fn assembles_spec_vector() {
        let t = tile(0, 0, 0, vec![iso_block(0, 0, 5), iso_block(64, 32, 6)]);
        let img = assemble(&t).unwrap();
        assert_eq!((img.x0, img.y0, img.width, img.height), (0, 0, 96, 47));
        assert_eq!(img.pixels[0], 5);
        assert_eq!(img.pixels[32 * 96 + 64], 6);
        assert_eq!(img.pixels[20 * 96 + 50], 0, "gap between blocks");
    }

    // Covers: specs/render/map-preview.md §tile-images
    #[test]
    fn negative_offsets() {
        let t = tile(1, 0, 0, vec![iso_block(16, -40, 3)]);
        let img = assemble(&t).unwrap();
        assert_eq!((img.x0, img.y0), (16, -40));
        assert!(assemble(&tile(1, 0, 0, vec![])).is_none());
    }

    // Covers: specs/render/map-preview.md §tile-library-and-lookup
    #[test]
    fn first_match_wins() {
        let dt1 = Dt1 {
            version: 7,
            minor_version: 6,
            tiles: vec![
                tile(0, 1, 2, vec![iso_block(0, 0, 7)]),
                tile(0, 1, 2, vec![iso_block(0, 0, 8)]),
                tile(0, 1, 3, vec![]),
            ],
        };
        let mut lib = TileLibrary::new();
        lib.add_dt1(&dt1);
        let key = |sub| TileKey {
            orientation: 0,
            main: 1,
            sub,
        };
        let i = lib.lookup(key(2)).unwrap().unwrap();
        assert_eq!(lib.images[i].pixels[0], 7);
        assert_eq!(lib.lookup(key(3)), Some(None));
        assert_eq!(lib.lookup(key(4)), None);
    }

    // Covers: specs/render/map-preview.md §tile-files
    #[test]
    fn tile_file_paths() {
        let ds1 = Ds1 {
            version: 18,
            width: 1,
            height: 1,
            act: 0,
            tag_type: 0,
            files: vec![
                br"\d2\data\global\tiles\act1\town\floor.tg1".to_vec(),
                br"C:\D2\DATA\GLOBAL\TILES\ACT1\TOWN\trees.tg1".to_vec(),
                b"nonsense".to_vec(),
            ],
            unknown_header: None,
            walls: vec![],
            orientations: vec![],
            floors: vec![],
            shadow: vec![],
            tags: None,
            objects: vec![],
            unknown_groups: None,
            groups: vec![],
            groups_truncated: false,
            paths: vec![],
            trailing: vec![],
        };
        assert_eq!(
            dt1_paths(&ds1),
            [
                r"data\global\tiles\act1\town\floor.dt1",
                r"DATA\GLOBAL\TILES\ACT1\TOWN\trees.dt1"
            ]
        );
    }
}
