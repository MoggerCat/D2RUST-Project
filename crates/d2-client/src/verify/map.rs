// Spec: specs/render/map-preview.md
//! Case kind `map`: today's map verify (Phase 1b), unchanged. CPU half:
//! `map::cpu` reference of the DS1 layout; GPU half: the palette-material
//! app in verify mode (`app::run`), which compares byte for byte and
//! writes `cpu.png`, `gpu.png` (and `diff.png`) to the output folder.
//! Until §A9 ports this case to the compute compositor, it does not go
//! through [`super::GpuCompositor`].

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use d2_formats::mpq::ArchiveSet;

use super::case::MapCase;
use super::{CaseReport, Status};
use crate::map::{self, cpu};

/// Largest texture side the verifier will request (wgpu's default limit).
pub const MAX_TEXTURE_SIDE: u32 = 8192;

/// The game archives in `$D2_GAME_DIR`.
pub fn archives() -> Result<ArchiveSet> {
    let dir = std::env::var("D2_GAME_DIR").context("set D2_GAME_DIR to the game folder")?;
    ArchiveSet::open_dir(&dir).with_context(|| format!("opening archives in {dir}"))
}

/// File name without folders and extension (`townN1`).
pub fn file_stem(ds1: &str) -> String {
    ds1.rsplit(['\\', '/'])
        .next()
        .and_then(|f| f.split('.').next())
        .unwrap_or("map")
        .to_owned()
}

/// The whole-map view, rounded up to even sizes (pixel alignment) and
/// clamped to the texture limit around the map center.
pub fn full_view(bounds: map::Bounds) -> cpu::View {
    let side = |lo: i32, hi: i32| -> (i32, u32) {
        let len = ((hi - lo) as u32).div_ceil(2) * 2;
        if len <= MAX_TEXTURE_SIDE {
            (lo, len)
        } else {
            let center = (lo + hi) / 2;
            (center - MAX_TEXTURE_SIDE as i32 / 2, MAX_TEXTURE_SIDE)
        }
    };
    let (left, width) = side(bounds.x0, bounds.x1);
    let (top, height) = side(bounds.y0, bounds.y1);
    cpu::View {
        left,
        top,
        width,
        height,
    }
}

/// Runs a map case; prints today's verify lines as it goes. `out`
/// defaults to `game/renders/verify-<ds1 stem>` (gitignored: the images
/// show game graphics).
pub fn run(name: &str, case: &MapCase, out: Option<PathBuf>, perturb: usize) -> CaseReport {
    let status = match run_inner(case, out, perturb) {
        Ok(status) => status,
        Err(e) => Status::Error(format!("{e:#}")),
    };
    CaseReport {
        name: name.to_owned(),
        kind: "map",
        lines: Vec::new(),
        status,
    }
}

fn run_inner(case: &MapCase, out: Option<PathBuf>, perturb: usize) -> Result<Status> {
    let archives = Arc::new(archives()?);
    let mut loaded = map::load(&archives, &case.ds1)?;
    let mut layout = map::build(&loaded.ds1, &loaded.library, case.wall_base);
    let (dcc, dc6) = map::load_sprites(&archives)?;
    map::add_sprites(&mut layout, &mut loaded.library, &loaded.ds1, &dcc, &dc6);
    let bounds = layout.bounds.context("map has nothing to draw")?;
    let view = match case.view {
        Some((left, top, width, height)) => cpu::View {
            left,
            top,
            width,
            height,
        },
        None => full_view(bounds),
    };
    anyhow::ensure!(
        view.width.is_multiple_of(2) && view.height.is_multiple_of(2),
        "--view width and height must be even (pixel alignment)"
    );
    println!(
        "verify {}: {} draw items, view {}x{} at {},{}",
        case.ds1,
        layout.items.len(),
        view.width,
        view.height,
        view.left,
        view.top
    );
    let mut reference = cpu::to_rgba(
        &cpu::render_indexed(&layout, &loaded.library, view),
        &loaded.palette,
    );
    super::perturb(&mut reference, perturb).map_err(anyhow::Error::msg)?;
    if perturb > 0 {
        println!("debug: corrupted {perturb} reference pixels; verify must FAIL");
    }
    let out_dir = out
        .unwrap_or_else(|| PathBuf::from(format!("game/renders/verify-{}", file_stem(&case.ds1))));
    let result = crate::app::run(
        archives,
        crate::app::MapConfig {
            ds1_path: case.ds1.clone(),
            wall_base: case.wall_base,
        },
        crate::app::Mode::Verify(crate::app::VerifyConfig {
            view,
            reference,
            out_dir: out_dir.clone(),
        }),
    );
    println!("images in {}", out_dir.display());
    Ok(match result {
        bevy::app::AppExit::Success => {
            println!("PASS: GPU render matches the CPU reference exactly");
            Status::Pass
        }
        bevy::app::AppExit::Error(code) => Status::Fail(format!("FAIL (exit code {code})")),
    })
}
