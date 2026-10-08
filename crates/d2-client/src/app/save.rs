// Spec: specs/formats/d2s.md (§1 writer, §7 stats), specs/formats/d2s-load.md
//! Saving the played character (stitch-save; `d2-client play`).
//!
//! The game writes the player's `.d2s` on window close and on an explicit
//! save. Where it goes: `play --save <file>` writes that file;
//! `play --new <class> <name>` writes `<name>.d2s` in a d2rs-own save
//! folder ([`default_save_dir`], or `--save-dir`; never inside the game
//! install).
//!
//! What a save holds today (d2rs-own, unverified): the character the
//! join loaded (a parsed save, or a fresh header for `--new`) with the
//! live values the load restores overlaid: the 16 base stats
//! ([`SNAPSHOT_STATS`]; `ActionCharacter::set_base_stat` loads them
//! back) and the quest flag records (`rest.quests`). Everything the sim
//! cannot yet hold or read back (skills, waypoints, items, mouse
//! skills, hotkeys, NPC fields: `ActionCharacter`'s unapplied steps)
//! passes through from the loaded save unchanged, and is empty for a new
//! character. The file's bytes are the `d2_formats::d2s::write` writer's.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use bevy::app::AppExit;
use bevy::prelude::{MessageWriter, Resource};
use d2_formats::d2s::{self, Body, D2s, Header, SaveTables, StatEntry, Stats};

use super::server_thread::ThreadLink;
use super::single_player::{self, Character, Link, Sim};
use crate::bridge::link::{LinkError, Pumped, SendQueue, Sent, ServerLink};

/// The base stats saved from the live player (`formats/d2s.md` §7:
/// strength .. goldbank, stat ids 0..=15).
pub const SNAPSHOT_STATS: [u16; 16] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15];
/// Stat id of the character level.
const LEVEL_STAT: u16 = 12;

/// Why a save did not happen.
#[derive(Debug, thiserror::Error)]
pub enum SaveError {
    #[error("{0}")]
    Path(String),
    #[error("no joined player to save")]
    NoPlayer,
    #[error("server thread: {0}")]
    Server(String),
    #[error("writing the save: {0}")]
    Format(String),
    #[error("{path}: {message}")]
    Io { path: String, message: String },
}

/// The d2rs-own save folder: `Documents/d2rs/saves` under the user's
/// profile (`USERPROFILE` on Windows, `HOME` elsewhere), else `./saves`.
// d2rs-own, unverified
pub fn default_save_dir() -> PathBuf {
    let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME"));
    match home {
        Some(h) => PathBuf::from(h)
            .join("Documents")
            .join("d2rs")
            .join("saves"),
        None => PathBuf::from("saves"),
    }
}

/// Where the character is written: `--save <file>` is that file;
/// `--new <class> <name>` is `<name>.d2s` in `save_dir` (default
/// [`default_save_dir`]). `None`: neither flag, so the default character
/// is not saved. A folder inside `game_dir` (the install) is refused, and
/// so is a `--new` file that already exists (it would overwrite a
/// character; load it with `--save`).
// d2rs-own, unverified
pub fn save_path(
    save: Option<&Path>,
    new: Option<&str>,
    save_dir: Option<&Path>,
    game_dir: Option<&Path>,
) -> Result<Option<PathBuf>, SaveError> {
    let inside = |p: &Path| {
        game_dir.is_some_and(|g| {
            let (p, g) = (
                std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf()),
                std::fs::canonicalize(g).unwrap_or_else(|_| g.to_path_buf()),
            );
            p.starts_with(g)
        })
    };
    let path = match (save, new) {
        (Some(file), _) => file.to_path_buf(),
        (None, Some(name)) => {
            let dir = save_dir.map_or_else(default_save_dir, Path::to_path_buf);
            let file = dir.join(format!("{name}.d2s"));
            if file.exists() {
                return Err(SaveError::Path(format!(
                    "{} already exists: load it with --save, or pick another name",
                    file.display()
                )));
            }
            file
        }
        (None, None) => return Ok(None),
    };
    if inside(&path) {
        return Err(SaveError::Path(format!(
            "{} is inside the game install: saves go elsewhere (--save-dir)",
            path.display()
        )));
    }
    Ok(Some(path))
}

/// The save the live values are laid over: a loaded save as it is, or
/// the stub of a new character (`formats/d2s.md` §2.6 defaults: the
/// writer's own header, one empty body) with its class and name.
// d2rs-own, unverified
pub fn base_save(character: &Character) -> D2s {
    match character {
        Character::Save(save, _) => (**save).clone(),
        Character::Named(c) => fresh(c.class, c.name(), c.difficulty),
        Character::New => fresh(
            single_player::PLAYER_CLASS as u8,
            single_player::PLAYER_NAME,
            single_player::GAME_SETUP.difficulty,
        ),
    }
}

fn fresh(class: u8, name: &[u8], difficulty: u8) -> D2s {
    let mut header = Header {
        class,
        ..Header::default()
    };
    let _ = header.set_name(name);
    // Not `status::NEW`: that flag marks the 335-byte stub (§2.6), and
    // this file has a body.
    header.status = if single_player::GAME_SETUP.expansion {
        d2s::status::EXPANSION
    } else {
        0
    };
    // Town per difficulty: act 0 of the current one (§2.5).
    header.towns[usize::from(difficulty).min(2)] = 0x80;
    // §1 rule 2, §8.4 rule 4, §8.5 rule 3: an expansion game's file always
    // ends `6A 66 6B 66 00` when there is no hireling and no golem.
    let expansion = header.status & d2s::status::EXPANSION != 0;
    D2s {
        header,
        body: Some(Body {
            skills: vec![0; usize::from(header_skill_count())],
            hireling_items: expansion.then_some(None),
            golem: expansion.then(d2s::Golem::default),
            ..Body::default()
        }),
    }
}

fn header_skill_count() -> u8 {
    Header::default().skill_count
}

fn now_secs() -> u32 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as u32)
}

/// What the running game says about the player at save time.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Live {
    /// `(stat id, base value)` for [`SNAPSHOT_STATS`].
    pub stats: Vec<(u16, i32)>,
    /// The quest flag records per difficulty (`QuestFlags::copy_out`).
    pub quests: Option<[[u8; 96]; 3]>,
    /// Items, skill levels and waypoints (q-save-full).
    pub extra: super::save_full::Extra,
    /// Mouse skills, town act, hireling and golem items (q-save-gaps).
    pub gaps: super::save_gaps::Gaps,
    /// The character is hardcore and the player is dying or dead
    /// ([`super::hardcore`]): the save gets the dead bit.
    pub hardcore_dead: bool,
    /// The character is hardcore.
    pub hardcore: bool,
}

/// Reads [`Live`] from the game's local player.
pub fn read_live(sim: &mut Sim) -> Result<Live, SaveError> {
    let (player, _) = single_player::local_player(sim).ok_or(SaveError::NoPlayer)?;
    let mut stats: Vec<(u16, i32)> = sim.events.action.with(&mut sim.game, |_, v| {
        SNAPSHOT_STATS
            .iter()
            .map(|&id| (id, v.stats.unit_base(player, id, 0)))
            .collect()
    });
    // A player always has level >= 1. Level 0 means the game holds no
    // stat table (the synthetic game): its zeros must not overwrite the
    // loaded character's stats.
    if stats.iter().any(|&(id, v)| id == LEVEL_STAT && v <= 0) {
        stats.clear();
    }
    let quests = sim.world.rest.quests.get(&player).map(|q| {
        let mut out = [[0u8; 96]; 3];
        for (rec, f) in out.iter_mut().zip(&q.flags) {
            *rec = f.copy_out();
        }
        out
    });
    let extra = super::save_full::read_extra(sim, player);
    let gaps = super::save_gaps::read_gaps(sim, player);
    let hardcore = sim.events.action.hooks().x.hardcore;
    // Player modes 0 (DT) and 17 (DD).
    let down = sim
        .events
        .action
        .sys
        .units
        .get(player)
        .is_some_and(|u| u.mode == 0 || u.mode == 17);
    Ok(Live {
        stats,
        quests,
        extra,
        gaps,
        hardcore,
        hardcore_dead: hardcore && down,
    })
}

/// `base` with the live values laid over it (the module docs list what).
// d2rs-own, unverified
pub fn apply_live(base: &D2s, live: &Live, now: u32) -> D2s {
    let mut save = base.clone();
    let body = save.body.get_or_insert_with(Body::default);
    match &mut body.stats {
        Stats::Bits(entries) => {
            for &(id, value) in &live.stats {
                match entries.iter_mut().find(|e| e.id == id && e.layer == 0) {
                    Some(e) => e.value = value,
                    None if value != 0 => entries.push(StatEntry {
                        id,
                        layer: 0,
                        value,
                    }),
                    None => {}
                }
            }
            entries.retain(|e| e.value != 0);
            entries.sort_by_key(|e| (e.id, e.layer));
        }
        Stats::Mask { values, .. } => {
            for &(id, value) in &live.stats {
                if let Some(v) = values.iter_mut().find(|v| v.0 == id) {
                    v.1 = value as u32;
                }
            }
        }
    }
    if let Some(records) = &live.quests {
        body.quests.records = *records;
    }
    super::save_full::apply_extra(body, &live.extra);
    super::save_gaps::apply_gaps(&mut save, &live.gaps);
    if let Some(&(_, level)) = live.stats.iter().find(|s| s.0 == LEVEL_STAT) {
        if level > 0 {
            save.header.level = level.min(99) as u8;
        }
    }
    if live.hardcore {
        save.header.status |= d2s::status::HARDCORE;
    }
    super::hardcore::mark_dead(&mut save, live.hardcore, live.hardcore_dead);
    // +0x2C stays as loaded: the game never sets the create time, so every
    // game-written save holds 0 there (§2.2 rule 10, edge case 11).
    save.header.save_time = now;
    save
}

/// Writes `save` to `path`: to a temporary file next to it, then renamed
/// over it; an existing file is first copied to `<path>.bak`.
pub fn write_file(path: &Path, save: &D2s, tables: &dyn SaveTables) -> Result<(), SaveError> {
    let io = |e: std::io::Error| SaveError::Io {
        path: path.display().to_string(),
        message: e.to_string(),
    };
    let bytes = d2s::write(save, tables).map_err(|e| SaveError::Format(e.to_string()))?;
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir).map_err(io)?;
    }
    if path.exists() {
        let mut bak = path.as_os_str().to_owned();
        bak.push(".bak");
        std::fs::copy(path, PathBuf::from(bak)).map_err(io)?;
    }
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    std::fs::write(&tmp, bytes).map_err(io)?;
    std::fs::rename(&tmp, path).map_err(io)
}

/// A [`ThreadLink`] the app and the [`SaveHandle`] both hold: every call
/// goes through the lock, in the caller's order.
pub struct SharedLink<C: d2_server::seams::Clock + Send + 'static>(Arc<Mutex<ThreadLink<Link<C>>>>);

impl<C: d2_server::seams::Clock + Send + 'static> SharedLink<C> {
    fn lock(&self) -> std::sync::MutexGuard<'_, ThreadLink<Link<C>>> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Runs `f` on the server thread's link between frames (tests, tools).
    pub fn with<R, F>(&self, f: F) -> Result<R, super::server_thread::ThreadStopped>
    where
        R: Send + 'static,
        F: FnOnce(&mut Link<C>) -> R + Send + 'static,
    {
        self.lock().with(f)
    }
}

impl<C: d2_server::seams::Clock + Send + 'static> Clone for SharedLink<C> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<C: d2_server::seams::Clock + Send + 'static> ServerLink for SharedLink<C> {
    fn protocol_version(&self) -> u32 {
        self.lock().protocol_version()
    }
    fn send(&mut self, queue: SendQueue, msg: &[u8]) -> Result<Sent, LinkError> {
        self.lock().send(queue, msg)
    }
    fn pump(&mut self) -> Result<Pumped, LinkError> {
        self.lock().pump()
    }
    fn receive(&mut self) -> Vec<Vec<u8>> {
        self.lock().receive()
    }
}

/// The save of the running game, callable from the app (a menu, the
/// window close). Cloning shares the same game and file.
#[derive(Clone, Resource)]
pub struct SaveHandle {
    path: PathBuf,
    save: Arc<dyn Fn() -> Result<(), SaveError> + Send + Sync>,
}

impl SaveHandle {
    /// The file a save writes.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Writes the character now.
    pub fn save(&self) -> Result<(), SaveError> {
        (self.save)()
    }
}

/// `link` shared with a [`SaveHandle`] that writes `path` with `base`
/// ([`base_save`]) and the `tables` of the game.
pub fn share<C: d2_server::seams::Clock + Send + 'static>(
    link: ThreadLink<Link<C>>,
    base: D2s,
    tables: Arc<dyn SaveTables + Send + Sync>,
    path: PathBuf,
) -> (SharedLink<C>, SaveHandle) {
    let shared = Arc::new(Mutex::new(link));
    let handle = shared.clone();
    let file = path.clone();
    let save = move || {
        let live = {
            let mut link = handle.lock().unwrap_or_else(|e| e.into_inner());
            link.with(|l| read_live(&mut l.host_mut().game))
                .map_err(|e| SaveError::Server(e.to_string()))??
        };
        write_file(&file, &apply_live(&base, &live, now_secs()), &*tables)
    };
    (
        SharedLink(shared),
        SaveHandle {
            path,
            save: Arc::new(save),
        },
    )
}

/// The Esc game menu's hook (stitch-hud): closes the game; `play::run`
/// saves the character when the app has stopped, on every way out.
pub fn request_save_and_exit(exit: &mut MessageWriter<AppExit>) {
    exit.write(AppExit::Success);
}
