// Spec: specs/sim/path-placement.md §11, §13; specs/client/model.md §11 rules 1, 3; specs/sim/intents-events.md §4 rule 3, §7.2; specs/formats/d2s-load.md
//! The single-player join of a client whose player is not yet placed: the
//! game part of the C→S 0x6B path (`0x0052C550` → `0x00530190`,
//! `path-placement.md` §13 rule 1), as far as the specs state it.
//!
//! [`enter_game`] queues, to the joining player's client and in this
//! order (`client/model.md` §11 rule 3, recorded join order):
//!
//! 1. S→C 0x59 AssignPlayer, part A of the player's add messages
//!    (`intents-events.md` §7.2: GUID, class, name, position): the player
//!    is not placed yet, so the position is (0, 0) as recorded;
//! 2. S→C 0x0B GameHandshake (type 0, the player's GUID);
//! 3. S→C 0x03 LoadAct (`model.md` §11 rule 1, builder `0x0053ABE0` →
//!    `0x0053B390`): the act, game +0x7C (the act DRLG's init seed), the
//!    act's town level id (act +0x08), game +0x80 (`dwObjSeed`);
//! 4. game entry `0x005394A0` (`path-placement.md` §11): S→C 0x07 for the
//!    spawn room of the act's town, the player put in the world, S→C 0x15
//!    with flag 1 (`d2_sim::wiring::path::place::game_entry`).
//!
//! The messages go through the action wiring's transport seam
//! (`Pending::send`), so the host queues them with the next tick's
//! messages, before them: the first tick's per-client update then runs
//! the room switch (`tick.md` §6 rule 5), which sends one S→C 0x07 per
//! room of the spawn room's adjacency array (`path-placement.md` §11
//! "Recipients").
//!
//! Not sent, because no spec gives them (named, not guessed):
//! - the messages between 0x0B and 0x03 in the recorded join (`model.md`
//!   §11 rule 3 lists 0x59, 0x0B, …, 0x03): the rest of `0x00530190`
//!   before the act creation is not specified;
//! - S→C 0x53 after 0x03 (`model.md` §11 rule 1: the act's environment
//!   fields from `0x0061C330`; layout "partial" in `server-messages.tsv`);
//! - part B of the player's add messages (`intents-events.md` §7.2:
//!   `0x005489F0`, `0x00534F80`, … are unspecified);
//! - the session messages of game creation (S→C 0x01, 0x00, 0x02) and
//!   0x04 (sent by the client pass once the client's room is ready,
//!   `tick.md` §6 rule 4; `0x0061A460` is not specified):
//!   `intents-events.md` open question 2.

use d2_formats::d2s::D2s;
use d2_proto::s2c::{AssignPlayer, GameHandshake};
use d2_proto::server::LoadAct;
use d2_sim::drlg::TOWN_LEVELS;
use d2_sim::units::UnitId;
use d2_sim::wiring::action::Pending;
use d2_sim::wiring::path::place::game_entry;
use d2_sim::wiring::path::walk::PathCtx;

use super::character::{self, ActionCharacter, CharacterWorld, LoadContext, LoadError, LoadReport};
use super::handlers::world::ActionEvents;
use super::SimGame;
use crate::seams::ClientId;

/// What the joining character brings (its save: `path-placement.md` §13
/// rule 2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Entry {
    /// The client's act byte (client +0x1AC): 0 for a new character.
    pub act: u8,
    /// The character name, zero-padded (player data; 0x59 bytes 6..22).
    pub name: [u8; 16],
}

/// Why the join could not run.
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum JoinError {
    #[error("client {0} has not joined")]
    NotJoined(ClientId),
    #[error("client {0} has no player unit")]
    NoPlayer(ClientId),
    #[error("player {0:?} is already in a room (game entry places an unplaced player)")]
    Placed(UnitId),
    #[error("act {0} has no DRLG (the act is created by the game creation path)")]
    NoAct(u8),
    #[error("the path provider is off (game entry places through it)")]
    NoPaths,
    /// A fatal assert of game entry (`path-placement.md` §11: no spawn
    /// room, no free point), as the wiring logged it.
    #[error("game entry failed: {0}")]
    Entry(String),
    /// The save's load failed (`formats/d2s.md` §10).
    #[error("character load: {0}")]
    Load(#[from] LoadError),
}

/// The S→C 0x03 of act `act` (`client/model.md` §11 rule 1).
pub fn load_act(act: u8, map_seed: u32, obj_seed: u32) -> LoadAct {
    let town = TOWN_LEVELS
        .get(usize::from(act))
        .copied()
        .unwrap_or_default();
    LoadAct {
        act,
        f2: map_seed,
        f6: town as u16,
        f8: obj_seed,
    }
}

/// The join of `client` (module docs). Returns the placed player.
///
/// Game +0x80 is the object control's `dwObjSeed`; a game built without
/// an object control (no game-creation sequence, `rng.md` §5.2 TODO in
/// `ActionSim::create_objects`) has none, and 0 is sent.
pub fn enter_game<D: ActionEvents, W>(
    s: &mut SimGame<D, W>,
    client: ClientId,
    entry: &Entry,
) -> Result<UnitId, JoinError> {
    if s.sim_client(client).is_none() {
        return Err(JoinError::NotJoined(client));
    }
    let player = s.player_of(client).ok_or(JoinError::NoPlayer(client))?;
    if s.game.lists.unit(player).and_then(|u| u.room()).is_some() {
        return Err(JoinError::Placed(player));
    }
    let a = s.events.action();
    if a.sys.hooks.paths.is_none() {
        return Err(JoinError::NoPaths);
    }
    let map_seed = a
        .sys
        .hooks
        .drlg
        .dungeon
        .acts
        .get(usize::from(entry.act))
        .and_then(Option::as_ref)
        .map(|d| d.init_seed)
        .ok_or(JoinError::NoAct(entry.act))?;
    let (class, guid) = a
        .sys
        .units
        .get(player)
        .map(|r| (r.class, r.guid))
        .ok_or(JoinError::NoPlayer(client))?;
    let obj_seed = a.sys.hooks.objects.as_ref().map_or(0, |o| o.obj_seed);
    let assign = AssignPlayer {
        guid,
        class: class as u8,
        name: entry.name,
        x: 0,
        y: 0,
    };
    let x = &mut a.sys.hooks.x;
    x.send(player, &assign.encode());
    x.send(
        player,
        &GameHandshake {
            unit_type: 0,
            unit_guid: guid,
        }
        .encode(),
    );
    x.send(player, &load_act(entry.act, map_seed, obj_seed).encode());
    let errors = a.sys.hooks.errors.len();
    let placed = a.with(&mut s.game, |g, v| {
        game_entry(PathCtx::of(v, g), player, entry.act)
    });
    if !placed {
        let why = a.sys.hooks.errors[errors..]
            .iter()
            .map(|e| format!("{e:?}"))
            .collect::<Vec<_>>()
            .join("; ");
        return Err(JoinError::Entry(why));
    }
    Ok(player)
}

/// The join of `client` with its character save (`formats/d2s-load.md`):
/// the load of `save` (parsed and checked by `d2_formats::d2s`) onto the
/// client's player unit on the action wiring ([`ActionCharacter`]), then,
/// for a full save, the caller's quest entry with mode 0 (load §2), then
/// the game entry ([`enter_game`]) in the act the save holds (§2.2 rule
/// 8; a new character enters act 0). Steps without a provider are listed
/// in the report.
///
/// TODO(formats/d2s-load.md §1): whether the caller's game entry also
/// runs the quest entry with mode 0 after a new-character start is not
/// stated; it is not run here.
pub fn enter_game_from_save<D: ActionEvents, W>(
    s: &mut SimGame<D, W>,
    client: ClientId,
    save: &D2s,
    ctx: &LoadContext,
) -> Result<(UnitId, LoadReport), JoinError> {
    if s.sim_client(client).is_none() {
        return Err(JoinError::NotJoined(client));
    }
    let player = s.player_of(client).ok_or(JoinError::NoPlayer(client))?;
    let report = s.events.action().with(&mut s.game, |_, v| {
        let mut cw = ActionCharacter { v, player };
        let mut r = character::load(save, ctx, &mut cw)?;
        if !r.new_character {
            if let Err(u) = cw.quest_entry(0) {
                r.unapplied.push(u);
            }
        }
        Ok::<_, LoadError>(r)
    })?;
    let entry = Entry {
        act: report.act,
        name: save.header.name,
    };
    let placed = enter_game(s, client, &entry)?;
    Ok((placed, report))
}
