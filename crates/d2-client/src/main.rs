//! d2-client entry point.
//!
//! Usage:
//!   d2-client view       [--ds1 PATH] [--wall-base N] [--frames N]
//!   d2-client verify     [--ds1 PATH] [--wall-base N] [--view L,T,W,H] [--out DIR]
//!   d2-client cpu-render [--ds1 PATH] [--wall-base N] [--view L,T,W,H] [--out FILE]
//!
//! `view` opens a window (pan: arrows/WASD, zoom: mouse wheel). `verify`
//! renders headlessly on the GPU and compares the result byte-for-byte with
//! the CPU reference renderer (default view: the whole map); exit code 0
//! means identical. `cpu-render` writes the CPU reference image only.
//!
//! Game files are read from $D2_GAME_DIR. Output images go under the
//! gitignored `game/` folder by default; they contain game graphics and must
//! never be committed.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{bail, Context, Result};
use d2_client::map::{self, cpu};
use d2_formats::mpq::ArchiveSet;

const DEFAULT_DS1: &str = r"data\global\tiles\ACT1\TOWN\townN1.ds1";
const DEFAULT_WALL_BASE: i32 = 80;

struct Options {
    ds1: String,
    wall_base: i32,
    view: Option<cpu::View>,
    out: Option<PathBuf>,
    probe: Option<(i32, i32)>,
    /// Debug: corrupt this many reference pixels, to prove `verify` fails.
    perturb: usize,
    /// `view`: close after this many frames (smoke test).
    frames: Option<u32>,
}

fn parse_view(s: &str) -> Result<cpu::View> {
    let parts: Vec<i64> = s
        .split(',')
        .map(|p| p.trim().parse::<i64>())
        .collect::<Result<_, _>>()
        .context("--view expects L,T,W,H")?;
    let [left, top, width, height] = parts[..] else {
        bail!("--view expects 4 numbers: L,T,W,H");
    };
    Ok(cpu::View {
        left: left as i32,
        top: top as i32,
        width: u32::try_from(width).context("width")?,
        height: u32::try_from(height).context("height")?,
    })
}

fn parse_options(args: &[String]) -> Result<Options> {
    let mut o = Options {
        ds1: DEFAULT_DS1.to_owned(),
        wall_base: DEFAULT_WALL_BASE,
        view: None,
        out: None,
        probe: None,
        perturb: 0,
        frames: None,
    };
    let mut it = args.iter();
    while let Some(flag) = it.next() {
        let mut value = || it.next().with_context(|| format!("{flag} needs a value"));
        match flag.as_str() {
            "--ds1" => o.ds1 = value()?.clone(),
            "--wall-base" => o.wall_base = value()?.parse().context("--wall-base")?,
            "--view" => o.view = Some(parse_view(value()?)?),
            "--out" => o.out = Some(PathBuf::from(value()?)),
            "--perturb" => o.perturb = value()?.parse().context("--perturb")?,
            "--frames" => o.frames = Some(value()?.parse().context("--frames")?),
            "--probe" => {
                let v = value()?;
                let (x, y) = v.split_once(',').context("--probe expects X,Y")?;
                o.probe = Some((x.trim().parse()?, y.trim().parse()?));
            }
            other => bail!("unknown option {other}"),
        }
    }
    Ok(o)
}

fn archives() -> Result<ArchiveSet> {
    let dir = std::env::var("D2_GAME_DIR").context("set D2_GAME_DIR to the game folder")?;
    ArchiveSet::open_dir(&dir).with_context(|| format!("opening archives in {dir}"))
}

fn file_stem(ds1: &str) -> String {
    ds1.rsplit(['\\', '/'])
        .next()
        .and_then(|f| f.split('.').next())
        .unwrap_or("map")
        .to_owned()
}

fn cpu_render(o: Options) -> Result<()> {
    let archives = archives()?;
    let mut loaded = map::load(&archives, &o.ds1)?;
    for f in &loaded.missing_files {
        eprintln!("warning: tile file not found: {f}");
    }
    let mut layout = map::build(&loaded.ds1, &loaded.library, o.wall_base);
    let (dcc, dc6) = map::load_sprites(&archives)?;
    map::add_sprites(&mut layout, &mut loaded.library, &loaded.ds1, &dcc, &dc6);
    let missing: usize = layout.missing.values().sum();
    println!(
        "{}: {}x{} cells, {} tile keys, {} draw items, {} cells with missing tiles ({} keys)",
        o.ds1,
        loaded.ds1.width,
        loaded.ds1.height,
        loaded.library.len(),
        layout.items.len(),
        missing,
        layout.missing.len()
    );
    for it in layout
        .items
        .iter()
        .filter(|it| it.source == map::layout::Source::Sprite)
    {
        let img = &loaded.library.images[it.image];
        let opaque = img.pixels.iter().filter(|&&p| p != 0).count();
        println!(
            "  sprite at ({}, {}) size {}x{}, {opaque} opaque pixels",
            it.x, it.y, img.width, img.height
        );
    }
    if let Some((x, y)) = o.probe {
        println!("probe at screen ({x}, {y}), topmost first:");
        for (i, it, p) in layout.probe(&loaded.library, x, y) {
            let c = loaded.palette.colors[usize::from(p)];
            let at = (it.cell.1 * loaded.ds1.width as i32 + it.cell.0) as usize;
            let raw = match it.source {
                map::layout::Source::Floor(l) => {
                    format!("floor cell {:#010x}", loaded.ds1.floors[l][at])
                }
                map::layout::Source::Wall(l) => format!(
                    "wall cell {:#010x}, orientation cell {:#x}",
                    loaded.ds1.walls[l][at], loaded.ds1.orientations[l][at]
                ),
                map::layout::Source::Sprite => String::new(),
            };
            println!("    raw {raw}");
            println!(
                "  item {i}: {:?} cell {:?} key o{} m{} s{} at ({}, {}); index {p} = rgb({}, {}, {})",
                it.source,
                it.cell,
                it.key.orientation,
                it.key.main,
                it.key.sub,
                it.x,
                it.y,
                c.r,
                c.g,
                c.b
            );
        }
        return Ok(());
    }
    let bounds = layout.bounds.context("map has nothing to draw")?;
    let view = o.view.unwrap_or(cpu::View {
        left: bounds.x0,
        top: bounds.y0,
        width: (bounds.x1 - bounds.x0) as u32,
        height: (bounds.y1 - bounds.y0) as u32,
    });
    let indexed = cpu::render_indexed(&layout, &loaded.library, view);
    let rgba = cpu::to_rgba(&indexed, &loaded.palette);
    let out = o.out.unwrap_or_else(|| {
        PathBuf::from(format!(
            "game/renders/{}-cpu-wb{}.png",
            file_stem(&o.ds1),
            o.wall_base
        ))
    });
    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent)?;
    }
    image::save_buffer(
        &out,
        &rgba,
        view.width,
        view.height,
        image::ExtendedColorType::Rgba8,
    )
    .with_context(|| format!("writing {}", out.display()))?;
    println!(
        "wrote {} ({}x{}, view at {},{})",
        out.display(),
        view.width,
        view.height,
        view.left,
        view.top
    );
    Ok(())
}

/// Largest texture side the verifier will request (wgpu's default limit).
const MAX_TEXTURE_SIDE: u32 = 8192;

/// The whole-map view, rounded up to even sizes (pixel alignment) and
/// clamped to the texture limit around the map center.
fn full_view(bounds: map::Bounds) -> cpu::View {
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

fn verify(o: Options) -> Result<()> {
    let archives = Arc::new(archives()?);
    let mut loaded = map::load(&archives, &o.ds1)?;
    let mut layout = map::build(&loaded.ds1, &loaded.library, o.wall_base);
    let (dcc, dc6) = map::load_sprites(&archives)?;
    map::add_sprites(&mut layout, &mut loaded.library, &loaded.ds1, &dcc, &dc6);
    let bounds = layout.bounds.context("map has nothing to draw")?;
    let view = o.view.unwrap_or_else(|| full_view(bounds));
    anyhow::ensure!(
        view.width.is_multiple_of(2) && view.height.is_multiple_of(2),
        "--view width and height must be even (pixel alignment)"
    );
    println!(
        "verify {}: {} draw items, view {}x{} at {},{}",
        o.ds1,
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
    let pixels = reference.len() / 4;
    if let Some(step) = pixels.checked_div(o.perturb) {
        // Spread the corrupted pixels evenly; flip the red channel's top bit.
        for i in (0..pixels).step_by(step.max(1)).take(o.perturb) {
            reference[i * 4] ^= 0x80;
        }
        println!(
            "debug: corrupted {} reference pixels; verify must FAIL",
            o.perturb
        );
    }
    let out_dir = o
        .out
        .unwrap_or_else(|| PathBuf::from(format!("game/renders/verify-{}", file_stem(&o.ds1))));
    let result = d2_client::app::run(
        archives,
        d2_client::app::MapConfig {
            ds1_path: o.ds1,
            wall_base: o.wall_base,
        },
        d2_client::app::Mode::Verify(d2_client::app::VerifyConfig {
            view,
            reference,
            out_dir: out_dir.clone(),
        }),
    );
    println!("images in {}", out_dir.display());
    match result {
        bevy::app::AppExit::Success => {
            println!("PASS: GPU render matches the CPU reference exactly");
            Ok(())
        }
        bevy::app::AppExit::Error(code) => bail!("FAIL (exit code {code})"),
    }
}

fn view(o: Options) -> Result<()> {
    let archives = Arc::new(archives()?);
    let result = d2_client::app::run(
        archives,
        d2_client::app::MapConfig {
            ds1_path: o.ds1,
            wall_base: o.wall_base,
        },
        d2_client::app::Mode::View {
            exit_after: o.frames,
        },
    );
    match result {
        bevy::app::AppExit::Success => Ok(()),
        bevy::app::AppExit::Error(code) => bail!("view exited with code {code}"),
    }
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("cpu-render") => cpu_render(parse_options(&args[1..])?),
        Some("verify") => verify(parse_options(&args[1..])?),
        Some("view") | None => view(parse_options(args.get(1..).unwrap_or(&[]))?),
        _ => bail!("usage: d2-client [view|verify|cpu-render] [--ds1 PATH] [--wall-base N] [--view L,T,W,H] [--out PATH]"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn view_parsing() {
        let v = parse_view("-10, 20,300,400").unwrap();
        assert_eq!((v.left, v.top, v.width, v.height), (-10, 20, 300, 400));
        assert!(parse_view("1,2,3").is_err());
        assert!(parse_view("1,2,-3,4").is_err());
    }

    #[test]
    fn stems() {
        assert_eq!(file_stem(DEFAULT_DS1), "townN1");
    }

    #[test]
    fn full_view_is_even_and_clamped() {
        let b = |x0, y0, x1, y1| map::Bounds { x0, y0, x1, y1 };
        let v = full_view(b(-3, -5, 10, 8));
        assert_eq!((v.left, v.top, v.width, v.height), (-3, -5, 14, 14));
        let v = full_view(b(0, 0, 20000, 100));
        assert_eq!((v.width, v.left), (MAX_TEXTURE_SIDE, 10000 - 4096));
    }
}
