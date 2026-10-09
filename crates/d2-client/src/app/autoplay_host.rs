// Spec: specs/tools/autoplay.md (§1, §2)
//! `d2-client autoplay-host`: the `play` client, headless, driven over a
//! line protocol on stdin / stdout, for the autoplay bot
//! (`tools/autoplay/`). The app is the one `d2-client play` runs
//! ([`super::play::add_live_client`] on Bevy's `MinimalPlugins`, as the
//! `play_smoke` test runs it): the server thread, the bridge, the
//! original UI and the world-click path. Only the window is missing; the
//! bot's input goes into the UI queue exactly where the window's pointer
//! and key events go (`world_view::present::ui_input` /
//! `script_input`), so a click passes the panels, the hover pick and the
//! world-click dispatcher like a player's.
//!
//! Nothing here sends a C→S message, pokes the game or moves a unit: the
//! commands are pointer and key events and reads (§1). The reads (`state`,
//! `map`) take the server snapshot of `state-dump` (`d2_sim::debug::state`)
//! and the DRLG's rooms on the server thread, reading only (CLAUDE.md
//! rule 6), and the client's UI and model.

use std::io::{BufRead, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use anyhow::{bail, Context, Result};
use bevy::prelude::*;
use serde_json::{json, Value};

use super::play::{add_live_client, LiveClient};
use super::play_start::{self, CliStart};
use super::server_thread::ThreadLink;
use super::single_player::{self, GameData, Link};
use super::state_dump::{StepClock, START_MS, STEP_MS};
use crate::bridge::items;
use crate::bridge::link::{LinkError, Pumped, SendQueue, Sent, ServerLink};
use crate::bridge::state::StateSource;
use crate::bridge::BridgeResource;
use crate::controls::Preset;
use crate::ui::edge;
use crate::ui::{FramePos, Point, PointerButton, UiEvent};
use crate::world_view::input_script::{play_key, vk_code};
use crate::world_view::WorldViewUi;

/// The protocol's format word (`autoplay-1`, §1 r1).
pub const FORMAT: &str = "autoplay-1";

/// The server thread, shared between the app's link and the reads.
type Server = Arc<Mutex<ThreadLink<Link<StepClock>>>>;

/// The C→S message ids the client sent, with counts (diagnostics).
type SentLog = Arc<Mutex<std::collections::BTreeMap<u8, u32>>>;

/// The app's link: the shared server thread, counting the C→S ids.
struct SharedLink(Server, SentLog);

impl ServerLink for SharedLink {
    fn protocol_version(&self) -> u32 {
        self.0.lock().expect("server lock").protocol_version()
    }
    fn send(&mut self, queue: SendQueue, msg: &[u8]) -> Result<Sent, LinkError> {
        if let Some(&id) = msg.first() {
            *self.1.lock().expect("sent lock").entry(id).or_default() += 1;
        }
        self.0.lock().expect("server lock").send(queue, msg)
    }
    fn pump(&mut self) -> Result<Pumped, LinkError> {
        self.0.lock().expect("server lock").pump()
    }
    fn receive(&mut self) -> Vec<Vec<u8>> {
        self.0.lock().expect("server lock").receive()
    }
}

/// `autoplay-host`'s options.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HostArgs {
    /// `--save FILE.d2s`; none: `--new`'s character.
    pub save: Option<PathBuf>,
    /// `--new CLASS NAME`.
    pub new: Option<(String, String)>,
    pub seed: Option<u32>,
    pub difficulty: u8,
    pub game_dir: Option<PathBuf>,
}

/// Parses the options after `autoplay-host`.
pub fn parse_args(args: &[String]) -> Result<HostArgs> {
    let mut a = HostArgs::default();
    let mut it = args.iter();
    while let Some(flag) = it.next() {
        let mut value = || it.next().with_context(|| format!("{flag} needs a value"));
        match flag.as_str() {
            "--save" => a.save = Some(PathBuf::from(value()?)),
            "--new" => {
                let class = value()?.clone();
                let name = value()?.clone();
                a.new = Some((class, name));
            }
            "--seed" => a.seed = Some(value()?.parse().context("--seed")?),
            "--difficulty" => {
                let v = value()?;
                a.difficulty = single_player::parse_difficulty(v)
                    .with_context(|| format!("--difficulty {v}"))?;
            }
            "--game-dir" => a.game_dir = Some(PathBuf::from(value()?)),
            other => bail!("autoplay-host: unknown option {other}"),
        }
    }
    if a.save.is_some() == a.new.is_some() {
        bail!("autoplay-host needs one of --save FILE or --new CLASS NAME");
    }
    Ok(a)
}

/// One command line (§1 r2).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Command {
    /// `step N`: N client passes, the clock 40 ms each.
    Step(u32),
    /// `move X Y`.
    Move(Point),
    /// `press L|R X Y` / `release L|R X Y`.
    Press(PointerButton, Point),
    Release(PointerButton, Point),
    /// `key K`: a key press through the bindings (`script_input`).
    Key(u32),
    /// `state`, `map`, `quit`.
    State,
    Map,
    Quit,
}

/// Parses one command line.
pub fn parse_command(line: &str) -> Result<Command, String> {
    let w: Vec<&str> = line.split_whitespace().collect();
    let num =
        |s: &str| -> Result<i32, String> { s.parse().map_err(|_| format!("`{s}`: not a number")) };
    let point = |x: &str, y: &str| -> Result<Point, String> {
        let (x, y) = (num(x)?, num(y)?);
        if !(0..800).contains(&x) || !(0..600).contains(&y) {
            return Err(format!("({x}, {y}) is outside the 800 × 600 frame"));
        }
        Ok(Point { x, y })
    };
    let button = |b: &str| match b {
        "L" | "l" => Ok(PointerButton::Left),
        "R" | "r" => Ok(PointerButton::Right),
        _ => Err(format!("`{b}`: L or R")),
    };
    match w.as_slice() {
        ["step", n] => Ok(Command::Step(
            n.parse().map_err(|_| format!("`{n}`: not a count"))?,
        )),
        ["move", x, y] => Ok(Command::Move(point(x, y)?)),
        ["press", b, x, y] => Ok(Command::Press(button(b)?, point(x, y)?)),
        ["release", b, x, y] => Ok(Command::Release(button(b)?, point(x, y)?)),
        ["key", k] => vk_code(k)
            .filter(|&vk| play_key(vk).is_some())
            .map(Command::Key)
            .ok_or_else(|| format!("unknown key `{k}`")),
        ["state"] => Ok(Command::State),
        ["map"] => Ok(Command::Map),
        ["quit"] => Ok(Command::Quit),
        _ => Err(format!("unknown command `{line}`")),
    }
}

/// The running host: the app, its server thread and its clock.
pub struct Host {
    app: App,
    server: Server,
    ms: Arc<AtomicU32>,
    passes: u64,
    sent: SentLog,
    /// Keys pressed since the last pass: held down for the next pass in
    /// the raw key state the window's input plugin keeps (the death and
    /// hardcore screens read it), released after it.
    keys: Vec<KeyCode>,
}

impl Host {
    /// The play app on the user's files, joined (the local player exists).
    pub fn start(args: &HostArgs) -> Result<Self> {
        let dir = args
            .game_dir
            .clone()
            .or_else(|| std::env::var_os("D2_GAME_DIR").map(PathBuf::from));
        let data = GameData::select(dir.as_deref())?;
        let start = play_start::resolve(
            &CliStart {
                save: args.save.clone(),
                new: args.new.clone(),
                save_dir: None,
                difficulty: args.difficulty,
                hardcore: false,
            },
            &data,
            dir.as_deref(),
            None,
            None,
        )?;
        let character = start.character;
        let seed = single_player::game_seed(&character, args.seed);
        let hardcore = start.hardcore
            || super::save::base_save(&character).header.status & d2_formats::d2s::status::HARDCORE
                != 0;
        let GameData::Live(live) = data.clone();
        let ms = Arc::new(AtomicU32::new(START_MS));
        let speeds = single_player::walk_speeds(&data, &character)?;
        let (mut link, started) =
            single_player::start_with(data, seed, character.clone(), StepClock(ms.clone()))?;
        if hardcore {
            link.with(|l| l.host_mut().game.events.action.hooks().x.hardcore = true)?;
        }
        let server: Server = Arc::new(Mutex::new(link));
        let sent = SentLog::default();
        let mut app = App::new();
        app.insert_resource(crate::bridge::mirror::ScriptedClock(ms.clone()));
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Image>()
            .init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<ButtonInput<KeyCode>>();
        add_live_client(
            &mut app,
            Box::new(SharedLink(server.clone(), sent.clone())),
            LiveClient {
                data: &live,
                request: &character,
                start_flags: start.start_flags,
                prices: started.prices,
                speeds,
                hardcore,
                automap_files: None,
                gpu: false,
            },
        )?;
        // The original preset, not the user's saved controls: the bot's
        // keys mean the same on every machine.
        {
            let mut ui = app.world_mut().non_send_mut::<WorldViewUi>();
            let b = Preset::Original.bindings();
            if let (Some(o), Some(b)) = (ui.original.as_mut(), b.as_ref()) {
                o.set_belt_keys(b);
            }
            ui.bindings = b;
        }
        eprintln!("autoplay-host: seed {seed}");
        let mut host = Host {
            app,
            server,
            ms,
            passes: 0,
            sent,
            keys: Vec::new(),
        };
        for _ in 0..2000 {
            if host.local_player().is_some() {
                break;
            }
            host.step(1);
        }
        if host.local_player().is_none() {
            bail!("the join did not run in 2000 passes");
        }
        Ok(host)
    }

    fn local_player(&self) -> Option<u32> {
        self.server
            .lock()
            .expect("server lock")
            .with(|l| single_player::local_player(&l.host().game).map(|(_, g)| g))
            .ok()
            .flatten()
    }

    /// N client passes, the clock 40 ms after each (one server tick).
    pub fn step(&mut self, n: u32) {
        for _ in 0..n {
            let keys = std::mem::take(&mut self.keys);
            {
                let mut input = self.app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
                input.clear();
                for &k in &keys {
                    input.press(k);
                }
            }
            self.app.update();
            {
                let mut input = self.app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
                for &k in &keys {
                    input.release(k);
                }
            }
            self.ms.fetch_add(STEP_MS, Ordering::SeqCst);
            self.passes += 1;
        }
    }

    fn ui(&mut self) -> Mut<'_, WorldViewUi> {
        self.app.world_mut().non_send_mut::<WorldViewUi>()
    }

    /// Queues pointer events as the window path does (`ui_input`): the
    /// cursor first, then the event; the cursor the click view reads.
    fn pointer(&mut self, at: Point, e: Option<UiEvent>) {
        let mut ui = self.ui();
        if ui.cursor() != Some(FramePos::Inside(at)) {
            ui.queue.0.push(UiEvent::CursorMoved(at));
            ui.set_cursor(FramePos::Inside(at));
        }
        ui.queue.0.extend(e);
    }

    /// A key press as `script_input` delivers a `key` step: the bound
    /// action in the UI's key mode, then the typed character.
    fn key(&mut self, vk: u32) {
        let Some(k) = play_key(vk) else { return };
        let codes: Vec<KeyCode> = edge::KEY_CODES
            .iter()
            .filter(|(_, e)| *e == k)
            .map(|(c, _)| *c)
            .take(1)
            .collect();
        self.keys.extend(codes.iter().copied());
        let mut ui = self.ui();
        if let Some(b) = ui.bindings.clone() {
            let mode = ui.original.as_ref().map_or(1, |o| o.key_mode());
            let actions = edge::key_actions_in_mode(&b, &codes, mode);
            ui.queue.0.extend(actions);
        }
        let chars = edge::key_chars(&codes);
        ui.queue.0.extend(chars);
    }

    /// Runs one command; the reply line (§1 r3), `None` for `quit`.
    pub fn run(&mut self, c: Command) -> Result<Option<Value>> {
        let ok = |what: &str| Ok(Some(json!({"k": "ok", "cmd": what})));
        match c {
            Command::Step(n) => {
                self.step(n);
                Ok(Some(
                    json!({"k": "ok", "cmd": "step", "passes": self.passes}),
                ))
            }
            Command::Move(p) => {
                self.pointer(p, None);
                ok("move")
            }
            Command::Press(button, at) => {
                self.pointer(at, Some(UiEvent::Press { button, at }));
                ok("press")
            }
            Command::Release(button, at) => {
                self.pointer(at, Some(UiEvent::Release { button, at }));
                ok("release")
            }
            Command::Key(vk) => {
                self.key(vk);
                ok("key")
            }
            Command::State => self.state().map(Some),
            Command::Map => self.map().map(Some),
            Command::Quit => Ok(None),
        }
    }

    /// `state` (§2 r1): the server snapshot and the client's view.
    fn state(&mut self) -> Result<Value> {
        let snap = self
            .server
            .lock()
            .expect("server lock")
            .state_snapshot()
            .map_err(|e| anyhow::anyhow!("{e:?}"))?;
        let snap: Value = serde_json::from_str(&snap.to_json_line())?;
        let local = self.local_player();
        let world = self.app.world();
        let w = &world.resource::<BridgeResource>().0.world().clone();
        let ui = world.non_send::<WorldViewUi>();
        // The drawn frame's camera anchor: the local player's position the
        // world view and the hover pick read (the walk prediction).
        let camera = world
            .get_resource::<crate::world_view::WorldViewState>()
            .and_then(|s| s.anchor)
            .map(|a| match a.player {
                crate::rules::camera::UnitPosition::Moving { x16, y16 } => {
                    json!({"x16": x16, "y16": y16, "x": x16 >> 16, "y": y16 >> 16, "shake": [a.shake.0, a.shake.1]})
                }
                crate::rules::camera::UnitPosition::Static { sx, sy } => {
                    json!({"x": sx, "y": sy, "shake": [a.shake.0, a.shake.1]})
                }
            });
        let o = ui.original.as_ref();
        let open: Vec<u8> = (0u8..0x30)
            .filter(|&i| o.is_some_and(|o| o.is_open(i)))
            .collect();
        let npc = o.and_then(|o| o.npc_menu()).map(|m| {
            let rows: Vec<Value> = m
                .rows
                .iter()
                .enumerate()
                .map(|(k, r)| {
                    let at = o.and_then(|o| o.npc_menu_row_point(k));
                    json!({
                        "string": r.string,
                        "kind": r.kind.map(|k| format!("{k:?}")),
                        "at": at.map(|p| [p.x, p.y]),
                    })
                })
                .collect();
            json!({"guid": m.guid, "class": m.class, "talking": m.talking, "rows": rows})
        });
        let topics = o.and_then(|o| o.npc_topics()).map(|t| {
            let pts: Vec<Value> = (0..t.len())
                .map(|k| json!(o.and_then(|o| o.npc_topic_point(k)).map(|p| [p.x, p.y])))
                .collect();
            json!({
                "text": t.iter().map(|s| String::from_utf16_lossy(s)).collect::<Vec<_>>(),
                "at": pts,
                "cancel": o.and_then(|o| o.npc_topic_cancel_point()).map(|p| [p.x, p.y]),
            })
        });
        let dialog = o.and_then(|o| o.npc_dialog_lines()).map(|l| l.len());
        let waypoint: Vec<Value> = o
            .map(|o| o.waypoint_rows())
            .unwrap_or_default()
            .iter()
            .map(|r| json!({"level": r.level, "known": r.known, "current": r.current}))
            .collect();
        let belt: Vec<Value> = items::belt(w)
            .values()
            .map(|i| json!({"slot": i.x, "code": code_str(i.code)}))
            .collect();
        let ground: Vec<Value> = items::ground_items(w)
            .iter()
            .map(|i| {
                json!({"g": i.key.guid, "code": code_str(i.code),
                       "x": i.x, "y": i.y, "gold": i.gold})
            })
            .collect();
        let me = w.local().map(
            |p| json!({"g": p.key.guid, "pos": p.position.map(|(x, y)| [x, y]), "mode": p.mode}),
        );
        Ok(json!({
            "k": "state",
            "format": FORMAT,
            "passes": self.passes,
            "server_ticks": w.server_ticks,
            "local": local,
            "client": {
                "player": me,
                "camera": camera,
                "act": w.act.as_ref().map(|a| format!("{a:?}")),
                "open": open,
                "npc_menu": npc,
                "topics": topics,
                "dialog_lines": dialog,
                "waypoint_rows": waypoint,
                "belt": belt,
                "ground": ground,
                "exit_requested": w.exit_requested,
                "sent": self.sent.lock().expect("sent lock").iter()
                    .map(|(k, v)| (format!("{k:02X}"), *v)).collect::<std::collections::BTreeMap<_, _>>(),
            },
            "snap": snap,
        }))
    }

    /// `map` (§2 r2): the allocated levels of the local player's act, with
    /// their rooms (sub-tile rects), near lists and warp links.
    fn map(&mut self) -> Result<Value> {
        let v = self
            .server
            .lock()
            .expect("server lock")
            .with(|l| {
                let sim = &mut l.host_mut().game;
                let (p, _) = single_player::local_player(sim)?;
                let act = sim.events.action.sys.units.get(p)?.act;
                let g = &mut sim.events;
                g.action
                    .hooks()
                    .drlg
                    .with_act(act, &mut sim.game.lists, |d, svc| {
                        let mut levels = Vec::new();
                        for id in 1..=136u32 {
                            let Some(li) = d.find_level(id) else { continue };
                            let lv = d.level(li);
                            let rooms: Vec<Value> = d
                                .level_rooms(li)
                                .into_iter()
                                .map(|r| {
                                    let room = d.room(r);
                                    let t = room.rect;
                                    let grid = room.active().map(|a| {
                                        let c = &a.collision;
                                        let cells: String = c.masks.iter().map(|&m| grid_char(m)).collect();
                                        json!({"rect": [c.rect.x, c.rect.y, c.rect.w, c.rect.h], "cells": cells})
                                    });
                                    json!({
                                        "grid": grid,
                                        "id": r.0,
                                        "rect": [t.x * 5, t.y * 5, t.w * 5, t.h * 5],
                                        "near": room.near().map(|n| n.iter().map(|x| x.0).collect::<Vec<_>>()),
                                        "warps": room.warp_links.iter().map(|w| json!({
                                            "to": w.target.0, "on": w.enabled, "row": w.lvlwarp_row,
                                        })).collect::<Vec<_>>(),
                                        "level": id,
                                    })
                                })
                                .collect();
                            let r = lv.rect;
                            levels.push(json!({
                                "id": id,
                                "type": lv.drlg_type,
                                "rect": [r.x * 5, r.y * 5, r.w * 5, r.h * 5],
                                "warp_centres": lv.warp_centres,
                                "vis": d.vis_array(svc.data, id).ok(),
                                "rooms": rooms,
                            }));
                        }
                        json!({"k": "map", "act": act, "levels": levels})
                    })
            })
            .map_err(|e| anyhow::anyhow!("{e:?}"))?;
        v.context("no local player or act")
    }
}

/// One sub-tile of a `map` grid (§2 r2): `#` blocks a walking player
/// (the player move mask 0x1C09 without the door bit), `D` a door, `.`
/// free.
fn grid_char(mask: u16) -> char {
    use d2_sim::drlg::collision::bits;
    if mask & bits::DOOR != 0 {
        'D'
    } else if mask & (0x1C09 & !bits::DOOR) != 0 {
        '#'
    } else {
        '.'
    }
}

/// An item code as text (`"hp1"`), empty when absent.
fn code_str(code: Option<[u8; 4]>) -> String {
    code.map(|c| String::from_utf8_lossy(&c).trim_end().to_owned())
        .unwrap_or_default()
}

/// `d2-client autoplay-host ...`: starts the game, prints the hello line
/// and serves commands until `quit` or the end of stdin.
pub fn serve(args: &[String]) -> Result<()> {
    let a = parse_args(args)?;
    let mut host = Host::start(&a)?;
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    writeln!(
        out,
        "{}",
        json!({"k": "hello", "format": FORMAT, "passes": host.passes})
    )?;
    out.flush()?;
    for line in std::io::stdin().lock().lines() {
        let line = line?;
        let t = line.trim();
        if t.is_empty() || t.starts_with('#') {
            continue;
        }
        let reply = match parse_command(t) {
            Ok(c) => match host.run(c) {
                Ok(Some(v)) => v,
                Ok(None) => break,
                // a read that cannot answer (no local player: the game
                // was left) is reported, the host keeps serving
                Err(e) => json!({"k": "error", "error": format!("{e:#}")}),
            },
            Err(e) => json!({"k": "error", "error": e}),
        };
        writeln!(out, "{reply}")?;
        out.flush()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn options_and_commands_parse() {
        let a = parse_args(&args(&["--new", "amazon", "Bot", "--seed", "7"])).unwrap();
        assert_eq!(a.new, Some(("amazon".into(), "Bot".into())));
        assert_eq!(a.seed, Some(7));
        assert!(parse_args(&args(&[])).is_err(), "a character is needed");
        assert!(parse_args(&args(&["--save", "a", "--new", "ama", "B"])).is_err());
        assert_eq!(parse_command("step 5"), Ok(Command::Step(5)));
        assert_eq!(
            parse_command("press L 400 300"),
            Ok(Command::Press(
                PointerButton::Left,
                Point { x: 400, y: 300 }
            ))
        );
        assert_eq!(
            parse_command("release r 1 2"),
            Ok(Command::Release(PointerButton::Right, Point { x: 1, y: 2 }))
        );
        assert_eq!(parse_command("key 1"), Ok(Command::Key(0x31)));
        assert_eq!(parse_command("key esc"), Ok(Command::Key(0x1B)));
        assert_eq!(parse_command("state"), Ok(Command::State));
        for bad in [
            "step",
            "step -1",
            "press X 1 2",
            "move 800 0",
            "key nope",
            "fly",
        ] {
            assert!(parse_command(bad).is_err(), "{bad}");
        }
    }
}
