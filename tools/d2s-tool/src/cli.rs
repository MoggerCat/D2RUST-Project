// Spec: specs/formats/d2s.md §1–§3 (read, write, checksum), §2.6 (stub)
//! The command line (hand-parsed: no argument crate in the workspace).

use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result};
use d2_formats::d2s::{self, status, D2s};

use crate::dump::dump;
use crate::save::{apply, new_save, parse_class, parse_difficulty, Edits};
use crate::tables::Tables;
use crate::{read_options, round_trip, RoundTrip};

pub const USAGE: &str = "\
d2s-tool: 1.14d character saves (.d2s) for local testing. Tables come from
--game-dir DIR or D2_GAME_DIR (needed for everything but new-stub and stub files).

  d2s-tool new --name N --class C [edit flags] -o OUT.d2s
  d2s-tool new-stub --name N --class C [--hardcore] [--expansion] [--time T] -o OUT.d2s
  d2s-tool dump FILE.d2s [--classic|--expansion]
  d2s-tool check FILE.d2s [--classic|--expansion]
  d2s-tool set IN.d2s -o OUT.d2s [edit flags]

class: ama|sor|nec|pal|bar|dru|ass or 0..6

edit flags:
  --level L                    level (new: creation stats + that level's experience; set: stats 12, 13)
  --expansion                  expansion character (status 0x20, jf/kf sections)
  --hardcore | --softcore      status 0x04
  --stat ID=VALUE              base stat as stored (life/mana/stamina 6-11 in 1/256 points); repeatable
  --gold G                     stat 14 (0..=level*10000)
  --skill INDEX=LEVEL          skill byte INDEX of the class list; repeatable
  --all-skills LEVEL           every skill byte
  --quests none|all|LIST       LIST: comma-separated [diff:]acts=N or [diff:]SLOT.BIT ('all' is Pending)
  --waypoints none|all|LIST    LIST: comma-separated [diff:]INDEX
  --difficulty-unlocked normal|nightmare|hell   progression bits (d2s.md §2.2 rule 5.4)
  --act A --difficulty D       town byte: act 0..4 of difficulty D (default 0, normal)
  --item CODE[@X,Y][:PAGE]     a normal identified item (page 0 inventory, 3 cube, 4 stash); repeatable
  --seed S                     game seed the items' seeds derive from (default 1)
  --map-seed S                 header map seed (new: default = the time)
  --time T                     create/save time (new: default now; set: save time)
";

/// Arguments after the subcommand.
struct Args {
    rest: Vec<String>,
    i: usize,
}

impl Args {
    fn next(&mut self) -> Option<String> {
        let a = self.rest.get(self.i).cloned();
        self.i += 1;
        a
    }

    fn value(&mut self, flag: &str) -> Result<String> {
        self.next().ok_or_else(|| anyhow!("{flag} needs a value"))
    }
}

fn num<T: std::str::FromStr>(s: &str, what: &str) -> Result<T> {
    let parsed = if let Some(h) = s.strip_prefix("0x") {
        u64::from_str_radix(h, 16)
            .ok()
            .and_then(|v| v.to_string().parse().ok())
    } else {
        s.parse().ok()
    };
    parsed.ok_or_else(|| anyhow!("{what}: bad number {s:?}"))
}

fn pair<A: std::str::FromStr, B: std::str::FromStr>(s: &str, what: &str) -> Result<(A, B)> {
    let (a, b) = s
        .split_once('=')
        .ok_or_else(|| anyhow!("{what}: want A=B, got {s:?}"))?;
    Ok((num(a, what)?, num(b, what)?))
}

/// Common options.
#[derive(Default)]
struct Common {
    game_dir: Option<PathBuf>,
    out: Option<PathBuf>,
    files: Vec<PathBuf>,
    expansion_read: Option<bool>,
}

/// Parses the flags of a subcommand into edits and common options.
fn parse(rest: Vec<String>) -> Result<(Edits, Common)> {
    let mut a = Args { rest, i: 0 };
    let mut e = Edits::default();
    let mut c = Common::default();
    while let Some(f) = a.next() {
        match f.as_str() {
            "--game-dir" => c.game_dir = Some(a.value(&f)?.into()),
            "-o" | "--out" => c.out = Some(a.value(&f)?.into()),
            "--classic" => c.expansion_read = Some(false),
            "--name" => e.name = Some(a.value(&f)?),
            "--class" => e.class = Some(parse_class(&a.value(&f)?)?),
            "--level" => e.level = Some(num(&a.value(&f)?, "--level")?),
            "--expansion" => {
                e.expansion = true;
                c.expansion_read = Some(true);
            }
            "--hardcore" => e.hardcore = Some(true),
            "--softcore" => e.hardcore = Some(false),
            "--stat" => e.stats.push(pair(&a.value(&f)?, "--stat")?),
            "--skill" => e.skills.push(pair(&a.value(&f)?, "--skill")?),
            "--all-skills" => e.all_skills = Some(num(&a.value(&f)?, "--all-skills")?),
            "--gold" => e.gold = Some(num(&a.value(&f)?, "--gold")?),
            "--quests" => e.quests = Some(a.value(&f)?.parse()?),
            "--waypoints" => e.waypoints = Some(a.value(&f)?.parse()?),
            "--difficulty-unlocked" => e.unlocked = Some(parse_difficulty(&a.value(&f)?)?),
            "--difficulty" => e.difficulty = Some(parse_difficulty(&a.value(&f)?)?),
            "--act" => e.act = Some(num(&a.value(&f)?, "--act")?),
            "--item" => e.items.push(a.value(&f)?.parse()?),
            "--seed" => e.seed = Some(num(&a.value(&f)?, "--seed")?),
            "--map-seed" => e.map_seed = Some(num(&a.value(&f)?, "--map-seed")?),
            "--time" => e.time = Some(num(&a.value(&f)?, "--time")?),
            s if s.starts_with('-') => bail!("unknown flag {s}\n\n{USAGE}"),
            _ => c.files.push(f.into()),
        }
    }
    Ok((e, c))
}

fn tables(c: &Common) -> Result<Tables> {
    let dir = match &c.game_dir {
        Some(d) => d.clone(),
        None => std::env::var_os("D2_GAME_DIR")
            .map(PathBuf::from)
            .context("tables needed: pass --game-dir DIR or set D2_GAME_DIR")?,
    };
    Tables::load_dir(&dir)
}

fn write_out(c: &Common, bytes: &[u8]) -> Result<PathBuf> {
    let p = c.out.clone().context("-o OUT.d2s is required")?;
    std::fs::write(&p, bytes).with_context(|| format!("writing {}", p.display()))?;
    Ok(p)
}

fn one_file(c: &Common) -> Result<&Path> {
    match c.files.as_slice() {
        [f] => Ok(f),
        _ => bail!("want exactly one input file\n\n{USAGE}"),
    }
}

/// Writes `save`, reads the bytes back and checks the rewrite is the same
/// (every file the tool writes passes its own `check`).
fn write_checked(save: &D2s, t: &Tables) -> Result<Vec<u8>> {
    let bytes = d2s::write(save, t).map_err(|e| anyhow!("write: {e}"))?;
    let opts = read_options(&bytes, None);
    match round_trip(&bytes, &opts, t)? {
        RoundTrip::Same(_) => Ok(bytes),
        RoundTrip::Differs { offset, .. } => {
            bail!("internal: the written save does not rewrite to itself (offset {offset:#x})")
        }
    }
}

/// Runs a command line (without the program name); returns the exit code.
pub fn run(args: &[String], out: &mut dyn Write) -> Result<i32> {
    let Some((cmd, rest)) = args.split_first() else {
        write!(out, "{USAGE}")?;
        return Ok(2);
    };
    let (e, c) = parse(rest.to_vec())?;
    match cmd.as_str() {
        "new" => {
            if !c.files.is_empty() {
                bail!("new takes no input file");
            }
            let t = tables(&c)?;
            let save = new_save(&e, &t)?;
            let bytes = write_checked(&save, &t)?;
            let p = write_out(&c, &bytes)?;
            writeln!(out, "wrote {} ({} bytes)", p.display(), bytes.len())?;
        }
        "new-stub" => {
            let name = e.name.as_deref().context("--name is required")?;
            let class = e.class.context("--class is required")?;
            let flags = if e.hardcore == Some(true) {
                status::HARDCORE
            } else {
                0
            } | if e.expansion { status::EXPANSION } else { 0 };
            let time = e.time.unwrap_or_else(|| {
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(0, |d| d.as_secs() as u32)
            });
            let stub = D2s::new_stub(name.as_bytes(), class, flags, time)
                .ok_or_else(|| anyhow!("name {name:?}: at most 15 bytes, no NUL"))?;
            // A stub has no stats or items: no table is read.
            let bytes = d2s::write(&stub, &NoTables).map_err(|e| anyhow!("write: {e}"))?;
            let p = write_out(&c, &bytes)?;
            writeln!(out, "wrote {} ({} bytes)", p.display(), bytes.len())?;
        }
        "dump" | "check" => {
            let f = one_file(&c)?;
            let bytes = std::fs::read(f).with_context(|| format!("reading {}", f.display()))?;
            let opts = read_options(&bytes, c.expansion_read);
            let stub = bytes.len() == d2s::HEADER_SIZE;
            let t = if stub { None } else { Some(tables(&c)?) };
            let st: &dyn d2s::SaveTables = match &t {
                Some(t) => t,
                None => &NoTables,
            };
            let rt = round_trip(&bytes, &opts, st)?;
            if cmd == "dump" {
                let save = match &rt {
                    RoundTrip::Same(s) | RoundTrip::Differs { save: s, .. } => s,
                };
                write!(out, "{}", dump(save, t.as_ref()))?;
                return Ok(0);
            }
            match rt {
                RoundTrip::Same(_) => {
                    writeln!(
                        out,
                        "{}: OK: {} bytes read in section order (expansion {}) and rewritten byte for byte",
                        f.display(),
                        bytes.len(),
                        opts.expansion
                    )?;
                }
                RoundTrip::Differs {
                    offset,
                    file_len,
                    rewrite_len,
                    ..
                } => {
                    writeln!(
                        out,
                        "{}: DIFFERS: first difference at offset {offset:#x} (file {file_len} bytes, rewrite {rewrite_len})",
                        f.display()
                    )?;
                    return Ok(1);
                }
            }
        }
        "set" => {
            let f = one_file(&c)?;
            let bytes = std::fs::read(f).with_context(|| format!("reading {}", f.display()))?;
            let t = tables(&c)?;
            let opts = read_options(&bytes, None);
            let mut save = d2s::read(&bytes, &opts, &t)
                .map_err(|e| anyhow!("read: {e} (load result {:?})", e.result()))?;
            apply(&mut save, &e, &t)?;
            let bytes = write_checked(&save, &t)?;
            let p = write_out(&c, &bytes)?;
            writeln!(out, "wrote {} ({} bytes)", p.display(), bytes.len())?;
        }
        "help" | "--help" | "-h" => write!(out, "{USAGE}")?,
        other => bail!("unknown command {other:?}\n\n{USAGE}"),
    }
    Ok(0)
}

/// Tables for a stub: never asked (a stub has no sections).
struct NoTables;

impl d2s::SaveTables for NoTables {
    fn stat_save(&self, _: u16) -> Option<d2s::StatSave> {
        None
    }
    fn item_entry_len(&self, _: &[u8]) -> Result<usize, String> {
        Err("no tables".to_owned())
    }
}
