//! d2-client entry point.
//!
//! Usage:
//!   d2-client [play]     [--seed N] [--frames N] [--synthetic] [--difficulty normal|nightmare|hell] [--save FILE.d2s | --new CLASS NAME [--save-dir DIR]] [--native DIR] [--source native|mpq]
//!   d2-client view       [--ds1 PATH] [--wall-base N] [--frames N]
//!   d2-client verify     [--case NAME]... [--cases DIR] [--perturb N]
//!   d2-client verify     [--ds1 PATH] [--wall-base N] [--view L,T,W,H] [--out DIR] [--perturb N]
//!   d2-client cpu-render [--ds1 PATH] [--wall-base N] [--view L,T,W,H] [--out FILE]
//!   d2-client play       ... --dump-draws DIR [--at-tick N]
//!   d2-client facts-compare ORIGINAL_DIR D2RS_DIR [--ignore COL,...]
//!
//! `play` (the default) opens a window running the local single-player game: the
//! in-process server (`d2-server` host over the wired `d2-sim`) pumped
//! once per frame through the bridge, the world view composed by the GPU
//! compositor's render-graph node. With $D2_GAME_DIR set it reads the
//! game's `levels` and `objects` tables and generates its levels from the
//! user's DS1 / DT1 files (unless `--synthetic`); otherwise it uses
//! synthetic tables and levels. `--save` joins with a character save
//! (read with the user's tables: needs $D2_GAME_DIR) instead of a new
//! sorceress. `--new CLASS NAME` joins with a new character of that class
//! (a name or 0–6: amazon, sorceress, necromancer, paladin, barbarian,
//! druid, assassin) and name (1–15 of A–Z, a–z, 0–9, `-`, `_`), held in
//! memory only: nothing is written to disk (decision D3, a stand-in for
//! the unspecified select / create screens).
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
//! `play --dump-draws DIR [--at-tick N]` writes the rendering facts
//! (`specs/tools/facts-render.md` §5) of the first drawn frame at server
//! tick N or later (default 1) to DIR and exits (skips the front end).
//! `facts-compare` reports the first difference between a 1.14d scene's
//! facts and a d2rs dump (§6): exit 0 match, 1 diverged, 2 partial, 3 error.
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
    /// `play --save`: the character save the join loads.
    save: Option<PathBuf>,
    /// `play --difficulty normal|nightmare|hell|0-2`.
    difficulty: u8,
    /// `play --new CLASS NAME`: a new character (decision D3).
    new: Option<(String, String)>,
    /// `play --native DIR`: a converted native folder (`native-assets.md` §3.4).
    native: Option<PathBuf>,
    /// `play --source native|mpq` (`mpq`: debug builds only, §5 r5).
    source: Option<String>,
    /// `play --new`: the folder the new character's `<name>.d2s` goes in.
    save_dir: Option<PathBuf>,
    hardcore: bool,
    /// `play --dump-draws DIR`: the facts export (`facts-render.md` §5).
    dump_draws: Option<PathBuf>,
    /// `play --at-tick N`: the dump's first server tick.
    at_tick: Option<u64>,
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
        difficulty: 0,
        save: None,
        new: None,
        native: None,
        source: None,
        save_dir: None,
        hardcore: false,
        dump_draws: None,
        at_tick: None,
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
            "--hardcore" => o.hardcore = true,
            "--dump-draws" => o.dump_draws = Some(PathBuf::from(value()?)),
            "--at-tick" => o.at_tick = Some(value()?.parse().context("--at-tick")?),
            "--save" => o.save = Some(PathBuf::from(value()?)),
            "--native" => o.native = Some(PathBuf::from(value()?)),
            "--source" => o.source = Some(value()?.clone()),
            "--difficulty" => {
                let v = value()?;
                o.difficulty =
                    d2_client::app::single_player::parse_difficulty(v).with_context(|| {
                        format!("--difficulty {v}: use normal, nightmare, hell or 0-2")
                    })?;
            }
            "--save-dir" => o.save_dir = Some(PathBuf::from(value()?)),
            "--new" => {
                let class = value()?.clone();
                let name = it
                    .next()
                    .context("--new needs a class and a name: --new CLASS NAME")?
                    .clone();
                o.new = Some((class, name));
            }
            "--probe" => {
                let v = value()?;
                let (x, y) = v.split_once(',').context("--probe expects X,Y")?;
                o.probe = Some((x.trim().parse()?, y.trim().parse()?));
            }
            other => bail!("unknown option {other}"),
        }
    }
    if o.save.is_some() && o.new.is_some() {
        bail!("--save and --new cannot be combined");
    }
    if o.at_tick.is_some() && o.dump_draws.is_none() {
        bail!("--at-tick needs --dump-draws DIR");
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

/// The game data of the options (`play`'s source choice).
fn select_data(
    o: &Options,
) -> Result<(
    d2_client::app::single_player::GameData,
    Option<PathBuf>,
    String,
)> {
    use d2_client::app::single_player;
    let choice = d2_client::assets::choose_source(
        o.native.as_deref(),
        o.source.as_deref(),
        |k| std::env::var(k).ok(),
        d2_client::assets::default_native_dir(),
    )
    .map_err(anyhow::Error::msg)?;
    let native = match &choice {
        d2_client::assets::SourceChoice::Native(dir) if !o.synthetic => Some(dir.clone()),
        _ => None,
    };
    let dir = std::env::var_os("D2_GAME_DIR").map(PathBuf::from);
    let data = match &native {
        Some(dir) => single_player::GameData::select_native(dir)?,
        None => single_player::GameData::select(dir.as_deref(), o.synthetic)?,
    };
    let origin = match &native {
        Some(dir) => format!("native folder {}", dir.display()),
        None => "D2_GAME_DIR".to_owned(),
    };
    Ok((data, dir, origin))
}

/// `play`: the front end (main menu) first, then the game; the game's window
/// closing returns to character select. `--new`, `--save` and `--frames` skip
/// the front end (a shortcut straight into the game).
fn play(o: Options) -> Result<()> {
    use d2_client::app::front_host::{run_front_end, FrontArt};
    use d2_client::app::front_start::{front_host, Entry, StartChoice};
    use d2_client::ui::front_end::Outcome;
    if o.save.is_some() || o.new.is_some() || o.frames.is_some() || o.dump_draws.is_some() {
        let (data, dir, origin) = select_data(&o)?;
        return play_once(&o, data, dir, origin, None, None);
    }
    let mut first = true;
    loop {
        let (data, dir, origin) = select_data(&o)?;
        let (art, expansion) = match &data {
            d2_client::app::single_player::GameData::Live(d) => {
                use d2_data::bin::TableFiles;
                let mut art = FrontArt::new(d.archives.source());
                if let Ok(t) = d2_client::app::strings::TableStrings::load(
                    d.archives.as_ref(),
                    d2_client::app::strings::LANG,
                ) {
                    art = art.with_strings(move |id| {
                        u16::try_from(id).map(|i| t.by_id(i)).unwrap_or_default()
                    });
                }
                (Some(art), d.archives.lod())
            }
            d2_client::app::single_player::GameData::Synthetic => (None, true),
        };
        let saves = o
            .save_dir
            .clone()
            .unwrap_or_else(d2_client::app::save::default_save_dir);
        // After a game: character select (§F1.3, REC-200).
        let entry = if first {
            Entry::First
        } else {
            Entry::AfterGame
        };
        let (host, handles) = front_host(&saves, art, expansion, entry);
        first = false;
        match run_front_end(host) {
            Outcome::Exit => return Ok(()),
            Outcome::GameLoad(g) => {
                let choice = StartChoice::resolve(g, &handles, &saves);
                play_once(&o, data, dir, origin, g.difficulty, choice)?
            }
        }
    }
}

fn play_once(
    o: &Options,
    data: d2_client::app::single_player::GameData,
    dir: Option<PathBuf>,
    origin: String,
    menu_difficulty: Option<u8>,
    choice: Option<d2_client::app::front_start::StartChoice>,
) -> Result<()> {
    use d2_client::app::{play, single_player};
    match &data {
        single_player::GameData::Live(d) => println!(
            "play: game data from {origin} ({} levels, {} objects, waypoint object class {}; level files: {} DS1, {} lvlsub DS1, {} DT1)",
            d.waypoints.levels.len(),
            d.waypoints.objects.len(),
            d.waypoints.object_class,
            d.files.ds1.0.len(),
            d.files.subs.0.len(),
            d.files.dt1.0.len()
        ),
        single_player::GameData::Synthetic => println!("play: synthetic tables and levels"),
    }
    // Before the window opens: a bad folder or a name taken stops here.
    let start = d2_client::app::play_start::resolve(
        &d2_client::app::play_start::CliStart {
            save: o.save.clone(),
            new: o.new.clone(),
            save_dir: o.save_dir.clone(),
            difficulty: o.difficulty,
            hardcore: o.hardcore,
        },
        &data,
        dir.as_deref(),
        menu_difficulty,
        choice.as_ref(),
    )?;
    if choice.is_some() || o.save.is_some() || o.new.is_some() {
        println!("play: {}", start.origin);
    }
    println!("play: difficulty {}", start.difficulty);
    let result = play::run(play::PlayConfig {
        data,
        seed: o.seed,
        character: start.character,
        exit_after: o.frames,
        save_path: start.save_path,
        start_flags: start.start_flags,
        hardcore: start.hardcore,
        dump: o
            .dump_draws
            .clone()
            .map(|dir| d2_client::facts::export::DumpRequest {
                dir,
                at_tick: o.at_tick.unwrap_or(1),
                command: std::env::args().collect::<Vec<_>>().join(" "),
            }),
    })?;
    match result {
        bevy::app::AppExit::Success => Ok(()),
        bevy::app::AppExit::Error(code) => bail!("play exited with code {code}"),
    }
}

/// `facts-compare ORIGINAL D2RS [--ignore COL,...]` (`facts-render.md`
/// §6): exit 0 match, 1 diverged, 2 partial, 3 error.
fn facts_compare(args: &[String]) -> i32 {
    let mut dirs = Vec::new();
    let mut ignore = Vec::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--ignore" => match it.next() {
                Some(v) => ignore.extend(v.split(',').map(|c| c.trim().to_owned())),
                None => {
                    eprintln!("facts-compare: --ignore needs a value");
                    return 3;
                }
            },
            _ => dirs.push(PathBuf::from(a)),
        }
    }
    let [original, d2rs] = &dirs[..] else {
        eprintln!("usage: d2-client facts-compare ORIGINAL_DIR D2RS_DIR [--ignore COL,...]");
        return 3;
    };
    match d2_client::facts::compare::compare_dirs(original, d2rs, &ignore) {
        Ok(outcome) => {
            println!("{outcome}");
            outcome.exit_code()
        }
        Err(e) => {
            eprintln!("facts-compare: ERROR: {e}");
            3
        }
    }
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("facts-compare") => std::process::exit(facts_compare(&args[1..])),
        Some("cpu-render") => cpu_render(parse_options(&args[1..])?),
        Some("verify") => verify(parse_options(&args[1..])?),
        Some("play") | None => play(parse_options(args.get(1..).unwrap_or(&[]))?),
        Some("view") => view(parse_options(&args[1..])?),
        _ => bail!("usage: d2-client [view|verify|cpu-render|play|facts-compare] [--ds1 PATH] [--wall-base N] [--view L,T,W,H] [--out PATH] [--case NAME] [--cases DIR] [--perturb N] [--seed N] [--frames N] [--synthetic] [--difficulty normal|nightmare|hell] [--save FILE.d2s | --new CLASS NAME [--save-dir DIR]] [--native DIR] [--source native|mpq] [--dump-draws DIR [--at-tick N]]"),
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
    fn new_takes_a_class_and_a_name() {
        let args = |a: &[&str]| a.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let o = parse_options(&args(&["--new", "amazon", "Test", "--frames", "3"])).unwrap();
        assert_eq!(o.new, Some(("amazon".to_owned(), "Test".to_owned())));
        assert_eq!(o.frames, Some(3));
        assert!(parse_options(&args(&["--new", "amazon"])).is_err());
        assert!(parse_options(&args(&["--new"])).is_err());
        assert!(parse_options(&args(&["--new", "0", "A", "--save", "x.d2s"])).is_err());
    }

    #[test]
    fn dump_draws_takes_a_dir_and_a_tick() {
        let args = |a: &[&str]| a.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let o = parse_options(&args(&["--dump-draws", "d", "--at-tick", "40"])).unwrap();
        assert_eq!(
            (o.dump_draws, o.at_tick),
            (Some(PathBuf::from("d")), Some(40))
        );
        assert!(parse_options(&args(&["--at-tick", "40"])).is_err());
        assert!(parse_options(&args(&["--dump-draws"])).is_err());
    }

    #[test]
    fn facts_compare_usage_errors_exit_3() {
        assert_eq!(facts_compare(&[]), 3);
        assert_eq!(facts_compare(&["a".into(), "--ignore".into()]), 3);
        assert_eq!(
            facts_compare(&["/no/such/a".into(), "/no/such/b".into()]),
            3
        );
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
