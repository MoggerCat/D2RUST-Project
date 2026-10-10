// Spec: specs/items/inventory-moves.md §6–§11; specs/sim/intents-events.md §2.3, §2.4, §3.2; specs/sim/tick.md §6; specs/audio/triggers-2.md §14; specs/world/cube.md §8
// Spec: specs/items/inventory.md (the sections other than §6–§11)
//! The item-move intents (C→S 0x16–0x29, 0x50, 0x61, 0x63; `inventory-moves.md`
//! §7) and the deferred item messages (§6, §11) on the wired host.
//!
//! A handler finds the client's player unit and hands the message to
//! `d2_sim::items::moves::handle` on the inventory wiring
//! (`d2_sim::wiring::inventory::InvDesk`: the inventory model, the unit
//! records and lists, the stat lists and the item store of the host's
//! one economy) through [`WorldHost::moves`]. Every check, result code
//! and state change is the module's; the handler maps the result to a
//! [`ResultCode`] and queues what the module sent "now" (§6.4: 0x63's
//! 0x9D / 0x9C, the 0x42 of a stack merge, the 0x3F of a targeting
//! reset) to the receivers' clients, in send order.
//!
//! The 0x9C / 0x9D / 0x7D messages are not the handler's: they leave in
//! the player unit update of the next client pass (§6.1).
//! [`update_pass`] runs it after `d2_sim::tick::tick` (see its reading).
//!
//! What no d2-sim module provides (positions and the free-spot search,
//! player data, item use, sockets, hirelings, sounds, the item bit
//! stream) is the host's [`MoveRest`] (`d2_sim::wiring::inventory::InvRest`
//! with its `MovePending` part).

use d2_sim::game::Game;
use d2_sim::items::inventory::InvTables;
use d2_sim::items::moves::{self as sim_moves, mode::GROUND, MoveFatal, MoveUnits, Owner, HANDLED};
use d2_sim::tick::EventDispatch;
use d2_sim::units::lifecycle::LifecycleHooks;
use d2_sim::units::sound::sound_message;
use d2_sim::units::UnitId;
use d2_sim::wiring::economy::Economy;
use d2_sim::wiring::inventory::{InvDesk, InvRest, InvState};

pub mod preview_skills;

use super::super::super::SimGame;
use super::super::world::{WorldError, WorldFault, WorldHost};
use super::result_code;
use crate::seams::{ClientId, Intents, MessageSink, ResultCode};

/// Every item-move C→S id: (id, `client-messages.tsv` name, owner spec
/// section). Exactly `d2_sim::items::moves::HANDLED`, in id order.
pub const MOVE_IDS: &[(u8, &str, &str)] = &[
    (0x16, "PickItem", "specs/items/inventory-moves.md §7.1"),
    (0x17, "DropItem", "specs/items/inventory-moves.md §7.2"),
    (
        0x18,
        "InsertItemInBuffer",
        "specs/items/inventory-moves.md §7.3",
    ),
    (
        0x19,
        "RemoveItemFromBuffer",
        "specs/items/inventory-moves.md §7.4",
    ),
    (0x1A, "EquipItem", "specs/items/inventory-moves.md §7.5"),
    (
        0x1B,
        "Swap2HandedItem",
        "specs/items/inventory-moves.md §7.6",
    ),
    (
        0x1C,
        "RemoveBodyItem",
        "specs/items/inventory-moves.md §7.7",
    ),
    (
        0x1D,
        "SwapCursorWithBody",
        "specs/items/inventory-moves.md §7.8",
    ),
    (0x1E, "Swap1HWith2H", "specs/items/inventory-moves.md §7.9"),
    (
        0x1F,
        "SwapCursorBufferItem",
        "specs/items/inventory-moves.md §7.10",
    ),
    (0x20, "UseGridItem", "specs/items/inventory-moves.md §7.11"),
    (0x21, "StackItems", "specs/items/inventory-moves.md §7.12"),
    (0x22, "UnstackItems", "specs/items/inventory-moves.md §7.13"),
    (0x23, "ItemToBelt", "specs/items/inventory-moves.md §7.14"),
    (0x24, "ItemFromBelt", "specs/items/inventory-moves.md §7.15"),
    (
        0x25,
        "SwitchBeltItem",
        "specs/items/inventory-moves.md §7.16",
    ),
    (0x26, "UseBeltItem", "specs/items/inventory-moves.md §7.17"),
    (
        0x27,
        "UseItemAction",
        "specs/items/inventory-moves.md §7.18",
    ),
    (0x28, "SocketItem", "specs/items/inventory-moves.md §7.19"),
    (0x29, "ScrollToBook", "specs/items/inventory-moves.md §7.20"),
    (0x50, "DropGold", "specs/items/inventory-moves.md §7.22"),
    (0x61, "MercItem", "specs/items/inventory-moves.md §7.23"),
    (
        0x63,
        "ItemToBeltShift",
        "specs/items/inventory-moves.md §7.24",
    ),
];

/// An item-move id (one [`MOVE_IDS`] row).
pub fn is_move_id(id: u8) -> bool {
    HANDLED.iter().any(|&(i, _)| i == id)
}

/// The inventory seams without a d2-sim provider, as the host holds
/// them: [`InvRest`] (with `MovePending`), plus the take of what
/// `MovePending::send` queued.
pub trait MoveRest: InvRest {
    /// The messages `MovePending::send` queued since the last take, in
    /// send order: (receiving unit, bytes).
    fn take_sent(&mut self) -> Vec<(Owner, Vec<u8>)>;

    /// The places of the game's players and items at the start of a
    /// move call, and the game's item format, for a rest that holds no
    /// positions of its own (the play preview's, [`StagedPlace`]).
    /// Default: ignored.
    fn stage(&mut self, _places: &[StagedPlace], _item_format: u16) {}

    /// The players' quest flag records (current difficulty) at the start
    /// of a move call, for a rest that holds no quest record of its own
    /// (`inventory-moves.md` §7.11 step 4). Default: ignored.
    fn stage_quest_flags(&mut self, _flags: &[(Owner, d2_sim::world::quests::QuestFlags)]) {}

    /// The quest flag writes the call made (`MovePending::set_quest_flag`)
    /// since the last take: (player, quest, flag, on). Default: none.
    fn take_quest_flag_writes(&mut self) -> Vec<(Owner, u8, u8, bool)> {
        Vec::new()
    }

    /// The ground items the call picked up, (player, item) in order
    /// (`MovePending::quest_item_picked`, hook ITEMPICKEDUP
    /// `0x00543D80`). Default: none.
    fn take_picked_items(&mut self) -> Vec<(Owner, d2_sim::items::moves::Guid)> {
        Vec::new()
    }

    /// The players' skill lists, lent for one move call (the ranged-throw
    /// test reads `tables`' item type equivalence).
    fn stage_skills(&mut self, _stage: preview_skills::SkillStage, _tables: &InvTables) {}

    /// The lent lists back, after the call.
    fn take_skills(&mut self) -> Option<preview_skills::SkillStage> {
        None
    }

    /// Messages made after the call, sent with the call's own.
    fn queue_sent(&mut self, _sent: Vec<(Owner, Vec<u8>)>) {}
}

/// Where a player or an item is when an item-move call starts: its
/// owner key, position in sub-tiles and room.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StagedPlace {
    pub owner: Owner,
    pub pos: (i32, i32),
    pub room: Option<d2_sim::units::RoomId>,
}

/// The item-move part of a game's world host: the inventory tables
/// (`InvTables::from_fixed`), the inventory state (one inventory per
/// owner unit, the item data fields this spec owns, the wiring's
/// errors) and the seams without a provider. The units, stats and items
/// are the host's economy.
pub struct InvParts {
    pub tables: InvTables,
    /// `InvState::errors` collects the wiring's provider errors; the
    /// host reads them (they are not handler results).
    pub state: InvState,
    pub rest: Box<dyn MoveRest + Send + Sync>,
}

impl InvParts {
    pub fn new(tables: InvTables, rest: Box<dyn MoveRest + Send + Sync>) -> Self {
        let mut state = InvState::new();
        state.equip_rules = true;
        // PROVISIONAL (REC-161): worn items attach their stats to the wearer.
        state.link_item_stats = true;
        Self {
            tables,
            state,
            rest,
        }
    }

    /// The inventory desk of one call over the host's economy
    /// (`InvDesk::new` fills the item data copies): the item moves, the
    /// update pass, and the vendor and cube adapters all run on it.
    pub fn desk<'d, 'a, H: LifecycleHooks>(
        &'d mut self,
        econ: &'d mut Economy<'a, H>,
    ) -> InvDesk<'d, 'a, H, dyn MoveRest + Send + Sync> {
        let InvParts {
            tables,
            state,
            rest,
        } = self;
        InvDesk::new(econ, tables, state, rest.as_mut())
    }
}

/// One call on the inventory wiring over the host's economy.
pub trait MoveCall {
    type Out;
    fn call<H: LifecycleHooks>(self, econ: &mut Economy<'_, H>, parts: &mut InvParts) -> Self::Out;
}

/// What the rest sent during a call, with the receiving units looked up
/// (`None`: a GUID without a unit; nothing is sent to it).
pub(crate) fn take_sent<H: LifecycleHooks>(
    d: &mut InvDesk<'_, '_, H, dyn MoveRest + Send + Sync>,
) -> Vec<(Option<UnitId>, Vec<u8>)> {
    let sent = d.rest.take_sent();
    sent.into_iter().map(|(o, b)| (d.unit_of(o), b)).collect()
}

/// One item-move message.
pub(crate) struct MoveRun<'m> {
    pub(crate) player: UnitId,
    pub(crate) msg: &'m [u8],
}

pub(crate) type MoveOut = (
    Option<Result<u32, MoveFatal>>,
    Vec<(Option<UnitId>, Vec<u8>)>,
    Vec<(UnitId, UnitId, bool)>,
);

/// The item use inside the handler (`items/use.md` §1) runs the Town
/// Portal cast on the economy's hooks (`LifecycleHooks::town_portal_cast`),
/// so a 0x20 / 0x26 / 0x27 charges the scroll or tome only when the cast
/// made the pair (`inventory-moves.md` §7.11 rule 3, §7.17).
impl MoveCall for MoveRun<'_> {
    type Out = MoveOut;
    fn call<H: LifecycleHooks>(self, econ: &mut Economy<'_, H>, parts: &mut InvParts) -> MoveOut {
        let mut d = parts.desk(econ);
        let guid = d.guid_of(self.player);
        let r = sim_moves::handle(&mut d, guid, self.msg);
        d.flush_equip();
        let walks = d.take_item_walks();
        (r, take_sent(&mut d), walks)
    }
}

/// The item part of the corpse creations the death queued
/// (`ActionHooks::death.loot`, `vitals.md` §4.7 rule 1.7): each corpse
/// gets an inventory, then the player's cursor and body items move onto
/// it ([`sim_moves::ground::corpse_fill`]). The result carries what the
/// rest sent.
pub struct CorpseFillRun {
    /// (player, corpse) with their GUIDs and the corpse's class.
    pub pairs: Vec<(Owner, Owner, u32)>,
    /// (player, amount) gold drops of the death penalty.
    pub gold: Vec<(Owner, i32)>,
}

impl MoveCall for CorpseFillRun {
    type Out = (Vec<MoveFatal>, Vec<(Option<UnitId>, Vec<u8>)>);
    fn call<H: LifecycleHooks>(self, econ: &mut Economy<'_, H>, parts: &mut InvParts) -> Self::Out {
        let mut faults = Vec::new();
        let mut d = parts.desk(econ);
        // `0x00535510`: piles near the player's death spot, the amount
        // already off the player's stat (the penalty set it).
        for (p, amount) in self.gold {
            sim_moves::ground::gold_piles(&mut d, p, amount, sim_moves::MAX_PILES);
        }
        for (p, c, class) in self.pairs {
            if let Some(cu) = d.unit_of(c) {
                d.state.add_inventory(
                    cu,
                    d2_sim::items::inventory::UnitKind::Player { class: class as u8 },
                    c.guid,
                );
            }
            if let Err(e) = sim_moves::ground::corpse_fill(&mut d, p, c) {
                faults.push(e);
            }
        }
        (faults, take_sent(&mut d))
    }
}

/// C→S 0x60 SwapWeapons. Its body (`0x005616A0`) is unwritten
/// (`intents-events.md` open question 16): d2rs-own, unverified, REC-177
/// ([`InvDesk::swap_weapon_sets`]). A host without the inventory parts
/// keeps the player handler's stub.
pub const SWAP_WEAPONS: u8 = 0x60;

struct SwapRun {
    player: UnitId,
}

impl MoveCall for SwapRun {
    type Out = (bool, Vec<(Option<UnitId>, Vec<u8>)>);
    fn call<H: LifecycleHooks>(self, econ: &mut Economy<'_, H>, parts: &mut InvParts) -> Self::Out {
        let mut d = parts.desk(econ);
        let owner = d.owner_of(self.player);
        let ok = owner.is_some_and(|o| d.swap_weapon_sets(o));
        (ok, take_sent(&mut d))
    }
}

fn swap_weapons<D: EventDispatch, W: WorldHost<D>>(
    sim: &mut SimGame<D, W>,
    client: ClientId,
    out: &mut dyn MessageSink,
) -> Option<ResultCode> {
    let player = sim.player_of(client)?;
    let (game, events) = (&mut sim.game, &mut sim.events);
    let (ok, sent) = sim.world.moves(game, events, SwapRun { player })?;
    if ok {
        sim.world.weapon_switched(events, player);
    }
    for (unit, bytes) in sent {
        if let Some(c) = unit.and_then(|u| sim.client_of(u)) {
            // A full queue is the host's fault elsewhere; the swap stands.
            let _ = out.queue(c, &bytes);
        }
    }
    Some(if ok {
        ResultCode::Done
    } else {
        ResultCode::Refused
    })
}

/// The handler of an item-move id after the dispatcher's gate and size
/// check (`intents-events.md` §2.3–§2.4). `None`: not an item-move id,
/// no player, or a host without the inventory parts, so the caller
/// keeps its stub. A fatal assert of the original (`MoveFatal`) or a
/// queueing failure is recorded with the host ([`WorldHost::fault`]) and
/// the handler returns [`ResultCode::Malformed`], as the world handlers
/// do.
pub fn handle<D: EventDispatch, W: WorldHost<D>>(
    sim: &mut SimGame<D, W>,
    client: ClientId,
    msg: &[u8],
    size: usize,
    out: &mut dyn MessageSink,
) -> Option<ResultCode> {
    let id = *msg.first()?;
    if id == SWAP_WEAPONS && size == 1 {
        return swap_weapons(sim, client, out);
    }
    // C→S 0x13 with unit type 4 (a ground item): `0x00548B00` case 4 is
    // the type-4 case of 0x16 (`inventory-moves.md` §7.1 step 2) with
    // cursor flag 0, so it runs as that message (`world/npc.md` §2).
    let as_pick;
    let msg = if id == 0x13 && size == 9 && msg.len() >= 9 && msg[1..5] == 4u32.to_le_bytes() {
        let mut m = vec![0x16];
        m.extend_from_slice(&msg[1..9]);
        m.extend_from_slice(&[0; 4]);
        as_pick = m;
        &as_pick[..]
    } else {
        msg
    };
    let (id, size) = if msg.len() == 13 && id == 0x13 {
        (0x16, 13)
    } else {
        (id, size)
    };
    if !is_move_id(id) {
        return None;
    }
    let player = sim.player_of(client)?;
    // Every item-move id has a fixed size ≤ 17 (`client-messages.tsv`).
    let msg = &msg[..size.min(msg.len())];
    let (game, events) = (&mut sim.game, &mut sim.events);
    let (run, sent, walks) = sim.world.moves(game, events, MoveRun { player, msg })?;
    // §7.1 step 2: a pick-up out of reach runs the player to the item.
    for w in walks {
        sim.world.item_walk(game, events, w);
    }
    let mut faults = Vec::new();
    for (unit, bytes) in sent {
        // §3.2 rule 1: a unit without a client receives nothing.
        if let Some(c) = unit.and_then(|u| sim.client_of(u)) {
            if let Err(e) = out.queue(c, &bytes) {
                faults.push(WorldError::from(e));
            }
        }
    }
    let result = match run? {
        Ok(r) => result_code(r),
        Err(e) => {
            faults.push(WorldError::Move(e));
            ResultCode::Malformed
        }
    };
    let failed = !faults.is_empty();
    for error in faults {
        sim.world.fault(WorldFault { client, id, error });
    }
    Some(if failed {
        ResultCode::Malformed
    } else {
        result
    })
}

/// One client's part of the update pass: the client's own player (the
/// owner test of §6.2) and the players whose unit update it processes.
#[derive(Clone)]
struct Receiver {
    client: ClientId,
    own: UnitId,
    players: Vec<UnitId>,
    /// Item units of the client's rooms not yet announced to it (the
    /// ground items' part of the unit update, §6.3 part 1).
    ground: Vec<UnitId>,
    /// Players whose item messages / sound already went out in the tick's
    /// queue walk ([`player_marked`]).
    items_done: Vec<UnitId>,
    sound_done: Vec<UnitId>,
}

/// The update pass of one tick.
#[derive(Clone)]
struct UpdateRun {
    receivers: Vec<Receiver>,
    /// Every player of the pass, for the clean-up.
    players: Vec<UnitId>,
    /// Ground items announced inside the tick's walk ([`GroundRun`]):
    /// their flags clear with the pass's.
    walked: Vec<UnitId>,
}

impl UpdateRun {
    /// The S→C 0x2C of each (receiver, player) of the pass, in pass order
    /// (`audio/triggers-2.md` §14 rule 2: target none or the receiver's
    /// player).
    fn sounds(&self, game: &Game) -> Vec<Option<[u8; 8]>> {
        self.receivers
            .iter()
            .flat_map(|r| {
                r.players.iter().map(move |&p| {
                    if r.sound_done.contains(&p) {
                        None
                    } else {
                        sound_message(game, p, r.own)
                    }
                })
            })
            .collect()
    }

    /// Unit flag 0x400 of every player of the pass := 0 (the room
    /// clean-up's step 3, `intents-events.md` §7.5, for the players; the
    /// tick wiring's clean-up leaves players' slots to this pass).
    fn clear_sounds(&self, game: &mut Game) {
        for &p in &self.players {
            game.sounds.clear(p);
        }
    }
}

type UpdateOut = (
    Vec<(ClientId, Vec<u8>)>,
    Vec<(ClientId, MoveFatal)>,
    Vec<(ClientId, UnitId)>,
);

impl MoveCall for UpdateRun {
    type Out = UpdateOut;
    fn call<H: LifecycleHooks>(self, econ: &mut Economy<'_, H>, parts: &mut InvParts) -> UpdateOut {
        // The sound of each (receiver, player), read before the pass: the
        // item messages do not touch the sound slots.
        let sounds = self.sounds(econ.game);
        let mut d = parts.desk(econ);
        // The items placed on the ground since the last pass: their unit
        // flag 0x1000 was cleared by the tick's room clean-up (PROVISIONAL,
        // REC-730: d2rs-own bookkeeping for the pass's place in the tick).
        let dropped = d.take_dropped();
        let (mut sent, mut fatal) = (Vec::new(), Vec::new());
        let mut announced = Vec::new();
        let mut sounds = sounds.into_iter();
        for r in &self.receivers {
            let own = d.guid_of(r.own);
            for &p in &r.players {
                let guid = d.guid_of(p);
                if !r.items_done.contains(&p) {
                    match sim_moves::player_update(&mut d, own, guid) {
                        Ok(msgs) => sent.extend(msgs.into_iter().map(|m| (r.client, m))),
                        Err(e) => fatal.push((r.client, e)),
                    }
                }
                // `cube.md` §8 rule 3: after the item messages and 0x47 /
                // 0x48, flag 0x400 → `0x00571740` (S→C 0x2C).
                if let Some(m) = sounds.next().flatten() {
                    sent.push((r.client, m.to_vec()));
                }
            }
            // The ground items of the client's rooms (§6.3 part 1, the
            // unit-add 0x9C of `0x00571F90`); d2rs-own, unverified.
            for &u in &r.ground {
                let guid = d.guid_of(u);
                if d.unit_exists(Owner::item(guid)) && d.mode(guid) == GROUND {
                    // An item dropped in this tick is announced with
                    // action 2 (§6.3, unit flag 0x1000; recorded
                    // 2026-10-09, `facts/items/a1-town-item-moves.tsv`
                    // n 57–58: a 0x17 drop from the cursor).
                    let was_dropped = dropped.contains(&u);
                    match sim_moves::announce_item_as(&d, guid, was_dropped) {
                        Ok(m) => sent.push((r.client, m)),
                        Err(e) => fatal.push((r.client, e)),
                    }
                    announced.push((r.client, u));
                }
            }
        }
        // The item case of the room clean-up (`intents-events.md` §7.5 step
        // 7, `generation.md` §1.4 row 0x2000): item flags 0x20 and 0x2000
        // clear once the item sat in a client's room queue, after the
        // announcing 0x9C carried them.
        for r in &self.receivers {
            for &u in &r.ground {
                if let Some(it) = d.econ.items.get_mut(u) {
                    it.flags &= !0x2020;
                }
            }
        }
        for &u in &self.walked {
            if let Some(it) = d.econ.items.get_mut(u) {
                it.flags &= !0x2020;
            }
        }
        // The update-list reset of the room clean-up (`tick.md` §3 step 6,
        // `0x00553220` → `0x00597B00`; §6.1 rule 4, `InvDesk::update_done`):
        // +0xC8 bit 0 cleared (bit 1, "save pending", stays: IS1), the
        // per-item resets, the update lists freed. The unit-flag part of
        // `0x00553220` runs in the tick wiring's step 6
        // (`d2_sim::wiring::action::View::room_cleanup`, `intents-events.md`
        // §7.5 step 3), before this pass.
        for &p in &self.players {
            let Some(o) = d.owner_of(p) else {
                continue;
            };
            if d.update_bits(o) & 1 == 0 {
                continue;
            }
            d.update_done(o);
        }
        // The desk's borrow of the economy ends here.
        self.clear_sounds(econ.game);
        (sent, fatal, announced)
    }
}

/// The deferred item messages of one tick (§6.1 rules 2–4): for each
/// client in client-list order (`unit-order.md` §7), for each player unit
/// in the client room's adjacent rooms (the rooms in array order, the
/// players of a room in client-list order), the player unit update
/// `0x00580860` (`items::moves::player_update`: with +0xC8 bit 0, the
/// update-list pass, then 0x47 and 0x48); the messages go to that
/// client. Then the clean-up of every player of the pass.
///
/// Reading: in 1.14d this runs inside the client pass (`tick.md` §6,
/// step 5) and the clean-up in step 6; here it runs after
/// `d2_sim::tick::tick`. The tick wiring's per-client unit update sends
/// no item message and its clean-up (`intents-events.md` §7.5, flags
/// only) clears no bit this pass reads (+0xC8 bits 0 and 1, the item and
/// command flags), so no step of the tick changes what this pass does. Every player
/// is queued for update by its own per-client update (`tick.md` §6 step
/// 5, last), so the queue membership test is the room test above. The
/// client's room is read after the tick's room switch (`0x00537B50`, in
/// the per-client update after the unit updates): in the tick of a
/// switch 1.14d walks the old room's adjacent rooms. The ground items' unit update (§6.3,
/// `d2_sim::items::moves::item_unit_update`) is not run: it belongs to the
/// per-unit update `0x0053A500` over the client's rooms, which the tick
/// wiring does not run for items; run here, after the tick, it would find
/// unit flags 0x1 and 0x10 already cleared by the room clean-up (IS2,
/// IS3).
pub fn update_pass<D: EventDispatch, W: WorldHost<D>>(
    sim: &mut SimGame<D, W>,
    out: &mut dyn MessageSink,
) {
    // An announced item that left the ground (picked up, freed: no room)
    // is forgotten, so its next landing is announced again (a drop from
    // the cursor, §6.3; PROVISIONAL REC-281, d2rs-own, unverified). The
    // freed pile's removal is the tick's S→C 0x0A.
    let lists = &sim.game.lists;
    sim.announced_ground
        .retain(|&(_, u)| lists.unit(u).is_some_and(|e| e.room().is_some()));
    let clients = sim.clients();
    let players: Vec<(UnitId, Option<d2_sim::units::RoomId>)> = clients
        .iter()
        .filter_map(|&c| sim.player_of(c))
        .map(|p| (p, sim.game.lists.unit(p).and_then(|u| u.room())))
        .collect();
    let mut receivers = Vec::new();
    for &c in &clients {
        let Some(own) = sim.player_of(c) else {
            continue;
        };
        let adjacent = sim
            .sim_client(c)
            .and_then(|id| sim.game.lists.client(id))
            .and_then(|e| e.room)
            .and_then(|r| sim.game.lists.room(r))
            .map(|r| r.adjacent.clone())
            .unwrap_or_default();
        let seen: Vec<UnitId> = adjacent
            .iter()
            .flat_map(|&room| {
                players
                    .iter()
                    .filter(move |&&(_, r)| r == Some(room))
                    .map(|&(p, _)| p)
            })
            .collect();
        let ground: Vec<UnitId> = adjacent
            .iter()
            .filter(|_| sim.announce_ground)
            .flat_map(|&room| sim.game.lists.room_units(room))
            .filter(|&u| {
                sim.game
                    .lists
                    .unit(u)
                    .is_some_and(|e| e.ty == d2_sim::units::UnitType::Item)
                    && !sim.announced_ground.contains(&(c, u))
            })
            .collect();
        receivers.push(Receiver {
            client: c,
            own,
            players: seen,
            ground,
            items_done: sim
                .walk_player_items
                .iter()
                .filter(|&&(cl, _)| cl == c)
                .map(|&(_, p)| p)
                .collect(),
            sound_done: sim
                .walk_player_sound
                .iter()
                .filter(|&&(cl, _)| cl == c)
                .map(|&(_, p)| p)
                .collect(),
        });
    }
    sim.walk_player_items.clear();
    sim.walk_player_sound.clear();
    let walked: Vec<UnitId> = std::mem::take(&mut sim.walk_announced)
        .into_iter()
        .map(|(_, u)| u)
        .collect();
    let run = UpdateRun {
        receivers,
        players: players.iter().map(|&(p, _)| p).collect(),
        walked,
    };
    let (game, events) = (&mut sim.game, &mut sim.events);
    let sound_only = run.clone();
    let Some((sent, fatal, announced)) = sim.world.moves(game, events, run) else {
        // A host without inventory parts: no item messages; the players'
        // sounds still leave (`cube.md` §8 rule 3) and are cleared.
        let mut sounds = sound_only.sounds(&sim.game).into_iter();
        for r in &sound_only.receivers {
            for _ in &r.players {
                let Some(m) = sounds.next().flatten() else {
                    continue;
                };
                if let Err(e) = out.queue(r.client, &m) {
                    sim.tick_faults.push((r.client, WorldError::from(e)));
                }
            }
        }
        sound_only.clear_sounds(&mut sim.game);
        return;
    };
    for (c, bytes) in sent {
        if let Err(e) = out.queue(c, &bytes) {
            sim.tick_faults.push((c, WorldError::from(e)));
        }
    }
    for (c, e) in fatal {
        sim.tick_faults.push((c, WorldError::Move(e)));
    }
    sim.announced_ground.extend(announced);
}

#[cfg(test)]
pub(crate) mod tests;

/// The announcement of one ground item at its place in the client pass's
/// queue walk (`inventory-moves.md` §6.3 part 1: the unit-add 0x9C of
/// `0x00571F90` inside the per-unit update).
struct GroundRun {
    item: UnitId,
}

impl MoveCall for GroundRun {
    type Out = Option<Result<Vec<u8>, MoveFatal>>;
    fn call<H: LifecycleHooks>(self, econ: &mut Economy<'_, H>, parts: &mut InvParts) -> Self::Out {
        let d = parts.desk(econ);
        let guid = d.guid_of(self.item);
        if !(d.unit_exists(Owner::item(guid)) && d.mode(guid) == GROUND) {
            return None;
        }
        let was_dropped = d.was_dropped(self.item);
        Some(sim_moves::announce_item_as(&d, guid, was_dropped))
    }
}

/// The host's answer to a ground-item mark of the tick's client pass
/// ([`d2_sim::wiring::action::GROUND_ITEM_MARK`]): the item's 0x9C to the
/// marked client now, so it stands where the queue walk put it. Only the
/// play host's pass announces ground items ([`SimGame::announce_ground`]).
pub fn announce_marked<D: EventDispatch, W: WorldHost<D>>(
    sim: &mut SimGame<D, W>,
    out: &mut dyn MessageSink,
    receiver: UnitId,
    guid: u32,
) {
    use d2_sim::units::UnitType;
    if !sim.announce_ground {
        return;
    }
    let Some(c) = sim.client_of(receiver) else {
        return;
    };
    let Some(item) = sim.game.lists.find_unit(UnitType::Item, guid) else {
        return;
    };
    if sim.announced_ground.contains(&(c, item))
        || sim.game.lists.unit(item).and_then(|e| e.room()).is_none()
    {
        return;
    }
    let (game, events) = (&mut sim.game, &mut sim.events);
    let Some(Some(r)) = sim.world.moves(game, events, GroundRun { item }) else {
        return;
    };
    match r {
        Ok(m) => {
            if let Err(e) = out.queue(c, &m) {
                sim.tick_faults.push((c, WorldError::from(e)));
            }
        }
        Err(e) => sim.tick_faults.push((c, WorldError::Move(e))),
    }
    sim.announced_ground.insert((c, item));
    sim.walk_announced.push((c, item));
}

/// One player's item messages (`0x00580860` step 2: the update-list pass,
/// 0x47, 0x48) at the player's place in the tick's queue walk.
struct PlayerItemsRun {
    own: UnitId,
    player: UnitId,
}

impl MoveCall for PlayerItemsRun {
    type Out = Result<Vec<Vec<u8>>, MoveFatal>;
    fn call<H: LifecycleHooks>(self, econ: &mut Economy<'_, H>, parts: &mut InvParts) -> Self::Out {
        let mut d = parts.desk(econ);
        let (own, guid) = (d.guid_of(self.own), d.guid_of(self.player));
        sim_moves::player_update(&mut d, own, guid)
    }
}

/// The host's answer to a player-update mark of the tick's client pass
/// ([`d2_sim::wiring::action::PLAYER_ITEMS_MARK`] /
/// [`d2_sim::wiring::action::PLAYER_SOUND_MARK`]): the item messages or the
/// sound of the player with `guid` to the marked client now, so they stand
/// where the queue walk put the player. The pass skips what went out here.
pub fn player_marked<D: EventDispatch, W: WorldHost<D>>(
    sim: &mut SimGame<D, W>,
    out: &mut dyn MessageSink,
    receiver: UnitId,
    guid: u32,
    sound: bool,
) {
    use d2_sim::units::UnitType;
    let Some(c) = sim.client_of(receiver) else {
        return;
    };
    let Some(player) = sim.game.lists.find_unit(UnitType::Player, guid) else {
        return;
    };
    if sound {
        if sim.walk_player_sound.contains(&(c, player)) {
            return;
        }
        if let Some(m) = sound_message(&sim.game, player, receiver) {
            if let Err(e) = out.queue(c, &m) {
                sim.tick_faults.push((c, WorldError::from(e)));
            }
        }
        sim.walk_player_sound.push((c, player));
        return;
    }
    if sim.walk_player_items.contains(&(c, player)) {
        return;
    }
    let (game, events) = (&mut sim.game, &mut sim.events);
    let Some(r) = sim.world.moves(
        game,
        events,
        PlayerItemsRun {
            own: receiver,
            player,
        },
    ) else {
        return;
    };
    match r {
        Ok(msgs) => {
            for m in msgs {
                if let Err(e) = out.queue(c, &m) {
                    sim.tick_faults.push((c, WorldError::from(e)));
                }
            }
        }
        Err(e) => sim.tick_faults.push((c, WorldError::Move(e))),
    }
    sim.walk_player_items.push((c, player));
}
