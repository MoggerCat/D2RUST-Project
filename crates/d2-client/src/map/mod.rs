// Spec: specs/render/map-preview.md
//! Map preview: DS1 + DT1 assembly and the CPU reference renderer. No Bevy
//! types here, so all of it is unit-testable and shared by the GPU path.

pub mod cpu;
pub mod layout;
pub mod tiles;

#[cfg(test)]
mod gaps_numbered_tests;

use d2_formats::ds1::Ds1;
use d2_formats::dt1::Dt1;
use d2_formats::mpq::ArchiveSet;
use d2_formats::palette::Palette;

pub use layout::{build, Bounds, DrawItem, Layout};
pub use tiles::{TileImage, TileKey, TileLibrary};

/// Everything needed to draw one preset map.
pub struct LoadedMap {
    pub ds1: Ds1,
    pub library: TileLibrary,
    pub palette: Palette,
    /// DT1 files listed by the DS1 but not found in the archives.
    pub missing_files: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    #[error("{path}: {source}")]
    Mpq {
        path: String,
        source: d2_formats::mpq::MpqError,
    },
    #[error("{path}: {source}")]
    Format {
        path: String,
        source: d2_formats::FormatError,
    },
}

/// Loads a DS1, its DT1 files and its act palette straight from the archives.
pub fn load(archives: &ArchiveSet, ds1_path: &str) -> Result<LoadedMap, LoadError> {
    let read = |path: &str| {
        archives.read(path).map_err(|source| LoadError::Mpq {
            path: path.to_owned(),
            source,
        })
    };
    let format_err = |path: &str| {
        let path = path.to_owned();
        move |source| LoadError::Format { path, source }
    };
    let ds1 = Ds1::parse(&read(ds1_path)?).map_err(format_err(ds1_path))?;
    let mut library = TileLibrary::new();
    let mut missing_files = Vec::new();
    for path in tiles::dt1_paths(&ds1) {
        if !archives.contains(&path) {
            missing_files.push(path);
            continue;
        }
        let dt1 = Dt1::parse(&read(&path)?).map_err(format_err(&path))?;
        library.add_dt1(&dt1);
    }
    let palette_path = palette_path(ds1.act);
    let palette = Palette::parse(&read(&palette_path)?).map_err(format_err(&palette_path))?;
    Ok(LoadedMap {
        ds1,
        library,
        palette,
        missing_files,
    })
}

/// The act palette for a DS1's 0-based act.
pub fn palette_path(act: u32) -> String {
    format!(r"data\global\palette\act{}\pal.dat", act.min(4) + 1)
}

/// Phase 1b demo sprites (spec §Sprites).
/// The Rogue Encampment bonfire (lit), body layer.
pub const DEMO_DCC: &str = r"data\global\objects\RB\TR\rbtrlitonhth.dcc";
pub const DEMO_DC6: &str = r"data\global\ui\PANEL\ctrlpnl7.DC6";

/// Adds the demo sprites on top of the map (spec §Sprites): frame 0 of the
/// DCC's direction 0 at the center cell's anchor, and the DC6's direction-0
/// frames in a row at the map's top-left corner.
pub fn add_sprites(
    layout: &mut Layout,
    lib: &mut TileLibrary,
    ds1: &Ds1,
    dcc: &d2_formats::dcc::Dcc,
    dc6: &d2_formats::dc6::Dc6,
) {
    let key = TileKey {
        orientation: u32::MAX,
        main: 0,
        sub: 0,
    };
    let mut push = |layout: &mut Layout, w: u32, h: u32, pixels: &[u8], x: i32, y: i32| {
        if w == 0 || h == 0 {
            return;
        }
        lib.images.push(TileImage {
            x0: 0,
            y0: 0,
            width: w,
            height: h,
            pixels: pixels.to_vec(),
            roof_height: 0,
        });
        layout.items.push(DrawItem {
            image: lib.images.len() - 1,
            x,
            y,
            key,
            cell: (-1, -1),
            source: layout::Source::Sprite,
        });
    };
    let corner = layout.bounds.map(|b| (b.x0, b.y0)).unwrap_or((0, 0));

    let (cx, cy) = (ds1.width as i32 / 2, ds1.height as i32 / 2);
    let (sx, sy) = layout::cell_origin(cx, cy);
    let (ax, ay) = (sx + layout::CELL_WIDTH / 2, sy + layout::CELL_HEIGHT / 2);
    if let Some(f) = dcc.directions.first().and_then(|d| d.frames.first()) {
        push(
            layout,
            f.width,
            f.height,
            &f.pixels,
            ax + f.x_min,
            ay + f.y_min,
        );
    }

    let frames = dc6.header.frames_per_direction as usize;
    let mut x = corner.0 + 32;
    for f in dc6.frames.iter().take(frames) {
        push(layout, f.width, f.height, &f.pixels, x, corner.1 + 32);
        x += f.width as i32;
    }
    layout.update_bounds(lib);
}

/// Loads the demo sprites straight from the archives.
pub fn load_sprites(
    archives: &ArchiveSet,
) -> Result<(d2_formats::dcc::Dcc, d2_formats::dc6::Dc6), LoadError> {
    let read = |path: &str| {
        archives.read(path).map_err(|source| LoadError::Mpq {
            path: path.to_owned(),
            source,
        })
    };
    let dcc =
        d2_formats::dcc::Dcc::parse(&read(DEMO_DCC)?).map_err(|source| LoadError::Format {
            path: DEMO_DCC.to_owned(),
            source,
        })?;
    let dc6 =
        d2_formats::dc6::Dc6::parse(&read(DEMO_DC6)?).map_err(|source| LoadError::Format {
            path: DEMO_DC6.to_owned(),
            source,
        })?;
    Ok((dcc, dc6))
}
