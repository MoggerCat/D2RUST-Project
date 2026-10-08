// Spec: specs/formats/d2s.md (§1 writer, §7 stats), specs/formats/d2s-load.md, specs/flows/save-exit.md (§1 r2, §2 r2, §3 r1, §4 r1)
//! Saving the played character (stitch-save; `d2-client play`).
//!
//! The server writes the player's `.d2s` (`flows/save-exit.md`): the
//! app installs a [`FileStore`] as the server's character storage, and
//! the server runs it in the leave of C→S 0x69 (Save and Exit,
//! [`request_save_and_exit`]) before its 0x05, and every 8192 frames.
//! Where it goes: `play --save <file>` writes that file;
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

use d2_server::adapters::storage::CharacterStore;
use d2_server::seams::ClientId;
use d2_sim::wiring::worldgen::WorldSim;

use super::server_thread::ThreadLink;
use super::single_player::{self, Character, Link, LocalSeams, Sim, World};
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
    /// The status bits the game set on the player's client
    /// (`ClientEntry::status_set`, `formats/d2s.md` §2.3: 0x08 at every
    /// death start, softcore too); the save ORs them in.
    pub status_set: u16,
    /// The game's map seed (game +0x7C, the DRLG init seed of act 0:
    /// `formats/d2s.md` §2.1 +0xAB); `None`: no DRLG (synthetic data).
    pub map_seed: Option<u32>,
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
    let status_set = sim
        .game
        .lists
        .clients()
        .into_iter()
        .filter_map(|c| sim.game.lists.client(c))
        .filter(|e| e.player == Some(player))
        .fold(0, |a, e| a | e.status_set);
    let map_seed = sim
        .events
        .action
        .sys
        .hooks
        .drlg
        .dungeon
        .acts
        .first()
        .and_then(Option::as_ref)
        .map(|d| d.init_seed);
    Ok(Live {
        map_seed,
        stats,
        quests,
        extra,
        gaps,
        hardcore,
        hardcore_dead: hardcore && down,
        status_set,
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
    // The client word's bits set in game (§2.3: the writer copies the
    // client word, which no code clears).
    save.header.status |= live.status_set;
    // +0xAB is game +0x7C (§2.1): the seed the game was built with.
    if let Some(seed) = live.map_seed {
        save.header.map_seed = seed;
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

/// The server's character writer of the app's game (`d2-server`
/// `adapters::storage`): the server runs it in the leave of C→S 0x69
/// before S→C 0x05 and every 8192 frames (`flows/save-exit.md` §2 r2,
/// §3 r1); it reads the running game ([`read_live`]), lays it over
/// `base` ([`apply_live`]) and writes `path` ([`write_file`]).
pub struct FileStore {
    pub path: PathBuf,
    pub base: D2s,
    pub tables: Arc<dyn SaveTables + Send + Sync>,
}

impl CharacterStore<WorldSim<LocalSeams>, World> for FileStore {
    fn save(&mut self, sim: &mut Sim, _client: ClientId) -> Result<(), String> {
        let live = read_live(sim).map_err(|e| e.to_string())?;
        write_file(
            &self.path,
            &apply_live(&self.base, &live, now_secs()),
            &*self.tables,
        )
        .map_err(|e| e.to_string())
    }
}

/// A save the app asks of the running game (the death saves,
/// [`super::hardcore`]): the server's `0x0052CA10` with its installed
/// [`FileStore`], run on the server thread between frames. Cloning
/// shares the same game and file.
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

/// Installs a [`FileStore`] that writes `path` with `base`
/// ([`base_save`]) and the `tables` of the game as the server's character
/// storage, and shares `link` with a [`SaveHandle`] over it.
pub fn share<C: d2_server::seams::Clock + Send + 'static>(
    mut link: ThreadLink<Link<C>>,
    base: D2s,
    tables: Arc<dyn SaveTables + Send + Sync>,
    path: PathBuf,
) -> Result<(SharedLink<C>, SaveHandle), SaveError> {
    let store = FileStore {
        path: path.clone(),
        base,
        tables,
    };
    link.with(move |l| l.host_mut().game.set_storage(Box::new(store)))
        .map_err(|e| SaveError::Server(e.to_string()))?;
    let shared = Arc::new(Mutex::new(link));
    let handle = shared.clone();
    let save = move || {
        let mut link = handle.lock().unwrap_or_else(|e| e.into_inner());
        let saved = link
            .with(|l| l.host_mut().game.save_characters())
            .map_err(|e| SaveError::Server(e.to_string()))?;
        match saved.into_iter().next() {
            None => Err(SaveError::NoPlayer),
            Some((_, r)) => r.map_err(|e| SaveError::Server(e.to_string())),
        }
    };
    Ok((
        SharedLink(shared),
        SaveHandle {
            path,
            save: Arc::new(save),
        },
    ))
}

/// The Esc game menu's Save and Exit Game (`flows/save-exit.md` §1 r2):
/// C→S 0x69 on the system queue and the model's `exit_requested`. The
/// server saves and answers 0x05, 0x06; [`end_of_game`] then closes the
/// app.
pub fn request_save_and_exit<L: ServerLink>(
    bridge: &mut crate::bridge::Bridge<L>,
) -> Result<(), crate::bridge::BridgeError> {
    bridge.save_and_exit().map(|_| ())
}

/// The client's end of the game (`flows/save-exit.md` §4 r1): the
/// server's 0x05 took the client out of the game (`in_game` false) and
/// the exit is asked (0x06, or the Save and Exit send): the app closes.
/// PROVISIONAL (`ui/frontend-menus.md` §F1.3, REC-200): the original
/// returns to character select; `play` has no front end around the game
/// yet, so it ends.
pub fn end_of_game(
    bridge: Option<bevy::prelude::Res<crate::bridge::BridgeResource>>,
    mut exit: MessageWriter<AppExit>,
    mut done: bevy::prelude::Local<bool>,
) {
    let Some(b) = bridge else {
        return;
    };
    let w = b.0.world();
    if !*done && w.exit_requested && !w.in_game {
        *done = true;
        exit.write(AppExit::Success);
    }
}
