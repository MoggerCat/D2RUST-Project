//! d2-client entry point.
//!
//! Usage:
//!   d2-client [play]     [--seed N] [--frames N] [--synthetic]
//!   d2-client view       [--ds1 PATH] [--wall-base N] [--frames N]
//!   d2-client verify     [--case NAME]... [--cases DIR] [--perturb N]
//!   d2-client verify     [--ds1 PATH] [--wall-base N] [--view L,T,W,H] [--out DIR] [--perturb N]
//!   d2-client cpu-render [--ds1 PATH] [--wall-base N] [--view L,T,W,H] [--out FILE]
//!
//! `play` (the default) opens a window running the local single-player game: the
//! in-process server (`d2-server` host over the wired `d2-sim`) pumped
//! once per frame through the bridge, the world view composed by the GPU
//! compositor's render-graph node. With $D2_GAME_DIR set it reads the
//! game's `levels` and `objects` tables and generates its levels from the
//! user's DS1 / DT1 files (unless `--synthetic`); otherwise it uses
//! synthetic tables and levels.
//! `view` opens a window (pan: arrows/WASD, zoom: mouse wheel). `verify`
//! runs the render cases (`crates/d2-client/render-cases/*.toml`, spec
//! `client/render-pipeline.md` §A10): per case, CPU reference vs GPU, byte
//! for byte; exit code 0 = all pass, 1 = a failure or error, 2 = none
//! failed but a GPU half is incomplete (no adapter) or a scene case stops
//! at a seam (the recording lacks what the world view needs; the recorded
//! camera is still checked). `verify --cases crates/d2-client/capture-cases` runs
//! the 1.14d capture cases (`render/capture.md`). With a map flag (`--ds1`,
//! `--wall-base`, `--view`, `--out`) it runs today's single-map verify
//! instead (default view: the whole map; exit 0 means identical).
//! `--perturb N` corrupts N reference pixels per case: each must fail with
//! exactly N. Every case (map and synthetic) runs on one headless compute
//! compositor. `cpu-render` writes the CPU reference image only.
//!
//! Game files are read from $D2_GAME_DIR. Output images go under the
//! gitignored `game/` folder by default; they contain game graphics and must
//! never be committed.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{bail, Context, Result};
use d2_client::map::{self, cpu};
use d2_client::verify::{
    self,
    map::{archives, file_stem},
};

const DEFAULT_DS1: &str = r"data\global\tiles\ACT1\TOWN\townN1.ds1";
const DEFAULT_WALL_BASE: i32 = 80;

struct Options {
    ds1: String,
    wall_base: i32,
    view: Option<cpu::View>,
    out: Option<PathBuf>,
    probe: Option<(i32, i32)>,
    /// Debug: corrupt this many reference pixels per case, to prove
    /// `verify` fails with exactly that count.
    perturb: usize,
    /// `verify`: a map flag (`--ds1`, `--wall-base`, `--view`, `--out`) was
    /// given, so run today's single-map verify instead of the case files.
    map_flags: bool,
    /// `verify`: case directory (default `crates/d2-client/render-cases`).
    case_dir: Option<PathBuf>,
    /// `verify`: run only these cases (file stems); default all.
    cases: Vec<String>,
    /// `view`, `play`: close after this many frames (smoke test).
    frames: Option<u32>,
    /// `play`: game seed.
    seed: u32,
    /// `play`: synthetic tables even with $D2_GAME_DIR set.
    synthetic: bool,
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
        map_flags: false,
        case_dir: None,
        cases: Vec::new(),
        frames: None,
        seed: d2_client::app::single_player::DEFAULT_SEED,
        synthetic: false,
    };
    let mut it = args.iter();
    while let Some(flag) = it.next() {
        let mut value = || it.next().with_context(|| format!("{flag} needs a value"));
        if matches!(flag.as_str(), "--ds1" | "--wall-base" | "--view" | "--out") {
            o.map_flags = true;
        }
        match flag.as_str() {
            "--case" => o.cases.push(value()?.clone()),
            "--cases" => o.case_dir = Some(PathBuf::from(value()?)),
            "--ds1" => o.ds1 = value()?.clone(),
            "--wall-base" => o.wall_base = value()?.parse().context("--wall-base")?,
            "--view" => o.view = Some(parse_view(value()?)?),
            "--out" => o.out = Some(PathBuf::from(value()?)),
            "--perturb" => o.perturb = value()?.parse().context("--perturb")?,
            "--frames" => o.frames = Some(value()?.parse().context("--frames")?),
            "--seed" => o.seed = value()?.parse().context("--seed")?,
            "--synthetic" => o.synthetic = true,
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

fn verify(o: Options) -> Result<()> {
    if o.map_flags {
        // Today's verify, unchanged: one map from the command line.
        anyhow::ensure!(
            o.cases.is_empty() && o.case_dir.is_none(),
            "--case/--cases cannot be combined with --ds1/--wall-base/--view/--out"
        );
        let case = verify::case::MapCase {
            ds1: o.ds1,
            wall_base: o.wall_base,
            view: o.view.map(|v| (v.left, v.top, v.width, v.height)),
        };
        let mut gpu = verify::gpu::Wgpu::new();
        let report = verify::map::run_with("map", &case, o.out, o.perturb, &mut gpu);
        verify::print_report(&report);
        return match report.status {
            verify::Status::Pass => Ok(()),
            verify::Status::Fail(why) => bail!(why),
            verify::Status::Error(e) => bail!(e),
            verify::Status::GpuNotWired => bail!("map case: GPU half not wired"),
            verify::Status::SceneNotWired => bail!("map case: scene source not wired"),
            verify::Status::NoAdapter(why) => {
                eprintln!("map case: no GPU adapter: {why}");
                std::process::exit(2)
            }
        };
    }
    let dir = o.case_dir.unwrap_or_else(verify::default_case_dir);
    let mut cases = verify::load_dir(&dir).map_err(anyhow::Error::msg)?;
    if !o.cases.is_empty() {
        for name in &o.cases {
            anyhow::ensure!(
                cases.iter().any(|c| &c.name == name),
                "no case {name} in {}",
                dir.display()
            );
        }
        cases.retain(|c| o.cases.contains(&c.name));
    }
    println!("verify: {} cases from {}", cases.len(), dir.display());
    // One compute compositor on a headless adapter for every case (map and
    // synthetic alike), so the process opens one device; opened at the
    // first case.
    let mut gpu = verify::gpu::Wgpu::new();
    let mut announced = false;
    let mut summary = verify::Summary::default();
    for case in &cases {
        println!("case {} ({})", case.name, case.kind.name());
        if !std::mem::replace(&mut announced, true) {
            println!("GPU compositor: {}", gpu.open());
        }
        let report = match &case.kind {
            verify::CaseKind::Synthetic(s) => {
                verify::run_synthetic(&case.name, s, o.perturb, &mut gpu)
            }
            verify::CaseKind::Map(m) => {
                verify::map::run_with(&case.name, m, None, o.perturb, &mut gpu)
            }
            // render/capture.md: the recorded camera is checked against
            // camera.md §1, §3, then the frame goes through the world view
            // and `rules::OriginalView`; d2rs does not yet read the draw
            // log into units, map and UI, so compare cases stop at that
            // seam (`scene_source::NotRecorded`).
            verify::CaseKind::Scene(sc) => verify::capture_case::run_capture(
                &case.name,
                sc,
                &verify::capture_case::repo_root(),
                o.perturb,
                &mut verify::capture_case::scene_source::WorldScene::new(
                    verify::capture_case::scene_source::NotRecorded,
                ),
                &mut gpu,
            ),
        };
        verify::print_report(&report);
        summary.add(&report.status);
    }
    println!("summary: {summary}");
    match summary.exit_code() {
        0 => Ok(()),
        code => std::process::exit(code),
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

fn play(o: Options) -> Result<()> {
    use d2_client::app::{play, single_player};
    let dir = std::env::var_os("D2_GAME_DIR").map(PathBuf::from);
    let data = single_player::GameData::select(dir.as_deref(), o.synthetic)?;
    match &data {
        single_player::GameData::Live(d) => println!(
            "play: game data from D2_GAME_DIR ({} levels, {} objects, waypoint object class {}; level files: {} DS1, {} lvlsub DS1, {} DT1)",
            d.waypoints.levels.len(),
            d.waypoints.objects.len(),
            d.waypoints.object_class,
            d.files.ds1.0.len(),
            d.files.subs.0.len(),
            d.files.dt1.0.len()
        ),
        single_player::GameData::Synthetic => println!("play: synthetic tables and levels"),
    }
    let result = play::run(play::PlayConfig {
        data,
        seed: o.seed,
        exit_after: o.frames,
    })?;
    match result {
        bevy::app::AppExit::Success => Ok(()),
        bevy::app::AppExit::Error(code) => bail!("play exited with code {code}"),
    }
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("cpu-render") => cpu_render(parse_options(&args[1..])?),
        Some("verify") => verify(parse_options(&args[1..])?),
        Some("play") | None => play(parse_options(args.get(1..).unwrap_or(&[]))?),
        Some("view") => view(parse_options(&args[1..])?),
        _ => bail!("usage: d2-client [view|verify|cpu-render|play] [--ds1 PATH] [--wall-base N] [--view L,T,W,H] [--out PATH] [--case NAME] [--cases DIR] [--perturb N] [--seed N] [--frames N] [--synthetic]"),
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
    fn map_flags_select_the_single_map_verify() {
        let args = |a: &[&str]| a.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert!(!parse_options(&[]).unwrap().map_flags);
        let o = parse_options(&args(&["--perturb", "3", "--case", "a"])).unwrap();
        assert!(!o.map_flags);
        assert_eq!((o.perturb, o.cases), (3, vec!["a".to_owned()]));
        for flag in [
            ["--ds1", "x.ds1"],
            ["--wall-base", "1"],
            ["--view", "0,0,2,2"],
            ["--out", "d"],
        ] {
            assert!(parse_options(&args(&flag)).unwrap().map_flags, "{flag:?}");
        }
    }
}
