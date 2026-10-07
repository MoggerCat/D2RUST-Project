// Spec: specs/sim/intents-events.md §8.1, §8.2; specs/sim/path-placement.md §11, §13; specs/client/model.md §11 rules 1, 3
//! The single-player session sequence of a client whose player is not
//! yet placed (`intents-events.md` §8): the game-creation messages of
//! C→S 0x67 (`0x00530BF0`, [`create_game`]) and the join of C→S 0x6B
//! (`0x0052C550` → `0x00530190`, [`enter_game`]), as far as the specs
//! state them. The app builds its game directly (no C→S 0x67 / 0x6B
//! path); these are the game parts of both handlers.
//!
//! [`create_game`] queues, to the client's player (§8.1 rules 3–6):
//! S→C 0x01 GameFlags, 0x00 (client state 1), 0x02.
//!
//! [`enter_game`] queues, in this order (§8.2 rules 3–6):
//!
//! 1. the player's own add messages (§7.2, rule 3.1): S→C 0x59
//!    AssignPlayer (GUID, class, name, (0, 0): the player is placed
//!    nowhere yet), then part B: 0xAA (its states), 0x76;
//! 2. S→C 0x0B GameHandshake (type 0, the player's GUID, rule 3.2);
//! 3. S→C 0x5F PortalFlags (rule 3.3), with the player record;
//! 4. S→C 0x7B for each hot-key slot whose skill is a `skills` row (rule
//!    3.6);
//! 5. two S→C 0x23 SetSkill, hand 1 then hand 0 (rule 3.7), with the
//!    player record;
//! 6. the join's vitals sync (rule 3.9): S→C 0x95, then the gold and
//!    experience messages against the client's cache;
//! 7. S→C 0x03 LoadAct (rule 4; `model.md` §11 rule 1, builder
//!    `0x0053ABE0` → `0x0053B390`): the act, game +0x7C (the act DRLG's
//!    init seed), the act's town level id (act +0x08), game +0x80
//!    (`dwObjSeed`); client state 2;
//! 8. game entry `0x005394A0` (rule 5, `path-placement.md` §11):
//!    S→C 0x07 for the spawn room of the act's town, the room switch
//!    (`intents-events.md` §7.8: 0x07 and the add messages for every room
//!    of the spawn room's adjacency array), the player put in the world,
//!    S→C 0x15 with flag 1, S→C 0x7E
//!    (`d2_sim::wiring::path::place::game_entry`);
//! 9. client state 3 (rule 6): the next tick's client pass sends 0x04
//!    once the client's room is ready (`tick.md` §6 rule 6).
//!
//! The messages go through the action wiring's transport seam
//! (`Pending::send`), so the host queues them with the next tick's
//! messages, before them.
//!
//! The values the join takes from the character (the save: name, act,
//! hot keys, the player record's portal flags and skill hands) are
//! [`Entry`]'s; the save loader is not wired (`path-placement.md` §13
//! rule 2), so a caller without a record gets no 0x5F and no 0x23.
//!
//! Not sent, because no spec gives them (named, not guessed):
//! - the loader's other messages after 0x76 (rule 3.1: 0x94, 0x22, 0x21,
//!   0x23, 0x5E, 0x28, 0x29 from the loader's callees);
//! - the player's stat messages (rules 3.4, 3.8: `0x006258D0` with the
//!   sender `0x00548520`, whose 0x1D / 0x1E / 0x1F choice
//!   (`0x0053BE40`) is not specified);
//! - the item messages of rule 3.5 and the update-list reset of rule 3.10
//!   (the item world is not reachable from the session), and
//!   `0x0058A0A0` of an expansion game;
//! - S→C 0x53 after 0x03 (rule 4: the three outputs of `0x0061C330`);
//! - followers (`path-placement.md` §13 rule 4).

use d2_proto::server::LoadAct;
use d2_sim::drlg::TOWN_LEVELS;
use d2_sim::units::lists::client_state;
use d2_sim::units::messages as msg;
use d2_sim::units::UnitId;
use d2_sim::wiring::action::switch::assign_player;
use d2_sim::wiring::action::vitals_sync;
use d2_sim::wiring::action::Pending;
use d2_sim::wiring::path::place::game_entry;
use d2_sim::wiring::path::walk::PathCtx;

use super::handlers::world::ActionEvents;
use super::SimGame;
use crate::seams::ClientId;

/// The game fields of S→C 0x01 (`intents-events.md` §8.1 rule 3).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GameSetup {
    /// Game +0x6D: 0 Normal, 1 Nightmare, 2 Hell.
    pub difficulty: u8,
    /// The arena record's flags (game +0x1D28 → +0x08; recorded
    /// 0x00100004 in every join of both recordings).
    pub arena_flags: u32,
    /// Game +0x70 ≠ 0.
    pub expansion: bool,
    /// Game +0x74 ≠ 0.
    pub ladder: bool,
}

/// One hot-key slot of the client record (client +0x3DC + 8i,
/// `intents-events.md` §8.2 rule 3.6).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HotKey {
    /// i16 at +0; a slot is sent only when 0 ≤ skill < the `skills` count.
    pub skill: i16,
    /// u8 at +2.
    pub flag: bool,
    /// u32 at +4.
    pub item: u32,
}

/// An unassigned slot (skill −1).
pub const NO_HOT_KEY: HotKey = HotKey {
    skill: -1,
    flag: false,
    item: u32::MAX,
};

/// One hand's skill of the player record (`0x006221A0(P)`: hand 0 at
/// +0x70 / +0x78, hand 1 at +0x74 / +0x7C; `intents-events.md` §8.2
/// rule 3.7).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SkillHand {
    pub skill: u16,
    pub item: u32,
}

/// The player record's fields the join sends (`0x006221A0(P)`, from the
/// save).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PlayerRecord {
    /// `0x00622230(P)`: S→C 0x5F (rule 3.3).
    pub portal_flags: u32,
    /// The skills of hand 0 and hand 1: the two S→C 0x23 (rule 3.7).
    pub hands: [SkillHand; 2],
}

/// What the joining character brings (its save: `path-placement.md` §13
/// rule 2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Entry {
    /// The client's act byte (client +0x1AC): 0 for a new character.
    pub act: u8,
    /// The character name, zero-padded (player data; 0x59 bytes 6..22).
    pub name: [u8; 16],
    /// The client's hot-key slots.
    pub hotkeys: [HotKey; 16],
    /// The player record's values. `None`: not given (no save loader is
    /// wired), so 0x5F and the two 0x23 are not sent.
    ///
    /// TODO(spec: formats/d2s.md, intents-events.md §8.2 rule 3): the
    /// record of a new character (`0x00532590`) is not specified.
    pub record: Option<PlayerRecord>,
}

impl Entry {
    /// A character with no hot key set and no player-record values.
    pub fn new(act: u8, name: [u8; 16]) -> Self {
        Self {
            act,
            name,
            hotkeys: [NO_HOT_KEY; 16],
            record: None,
        }
    }
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

/// The game-creation messages of `client` (`intents-events.md` §8.1
/// rules 3–6): S→C 0x01, 0x00 (client state := 1), 0x02, to the client's
/// player.
pub fn create_game<D: ActionEvents, W>(
    s: &mut SimGame<D, W>,
    client: ClientId,
    setup: &GameSetup,
) -> Result<(), JoinError> {
    let id = s.sim_client(client).ok_or(JoinError::NotJoined(client))?;
    let player = s.player_of(client).ok_or(JoinError::NoPlayer(client))?;
    let x = &mut s.events.action().sys.hooks.x;
    x.send(
        player,
        &msg::game_flags(
            setup.difficulty,
            setup.arena_flags,
            setup.expansion,
            setup.ladder,
        ),
    );
    x.send(player, &msg::GAME_LOADING);
    if let Some(e) = s.game.lists.client_mut(id) {
        e.state = client_state::LOADING;
    }
    s.events
        .action()
        .sys
        .hooks
        .x
        .send(player, &msg::LOAD_SUCCESSFUL);
    Ok(())
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
    let id = s.sim_client(client).ok_or(JoinError::NotJoined(client))?;
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
    let skills = a.sys.hooks.tables.skills.skills.len();
    a.sys.hooks.session.names.insert(player, entry.name);
    // Rule 3.1: the player's own add messages, placed nowhere.
    a.sys
        .hooks
        .x
        .send(player, &assign_player(guid, class as u8, &entry.name, 0, 0));
    a.with(&mut s.game, |g, v| v.player_part_b(g, player, player));
    // Rules 3.2–3.7.
    let x = &mut a.sys.hooks.x;
    x.send(player, &msg::unit_ref(0x0B, 0, guid));
    if let Some(r) = &entry.record {
        x.send(player, &msg::portal_flags(r.portal_flags));
    }
    for (i, k) in entry.hotkeys.iter().enumerate() {
        if usize::try_from(k.skill).is_ok_and(|sk| sk < skills) {
            x.send(
                player,
                &msg::assign_hotkey(i as u8, k.skill, k.flag, k.item),
            );
        }
    }
    if let Some(r) = &entry.record {
        for hand in [1u8, 0] {
            let h = r.hands[usize::from(hand)];
            x.send(player, &msg::set_skill(0, guid, hand, h.skill, h.item));
        }
    }
    // Rule 3.9.
    for m in vitals_sync::join_run(a, &s.game, id, (0, 0)) {
        a.sys.hooks.x.send(player, &m);
    }
    // Rule 4.
    a.sys
        .hooks
        .x
        .send(player, &load_act(entry.act, map_seed, obj_seed).encode());
    if let Some(e) = s.game.lists.client_mut(id) {
        e.state = client_state::ACT_LOADED;
    }
    // Rule 5.
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
    // Rule 6.
    if let Some(e) = s.game.lists.client_mut(id) {
        e.state = client_state::JOINING;
    }
    Ok(player)
}
