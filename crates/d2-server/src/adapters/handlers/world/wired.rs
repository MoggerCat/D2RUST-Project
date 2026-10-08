// Spec: specs/world/npc.md §1.1, §2–§4, §7.5; specs/world/vendors.md §1, §3, §4, §7; specs/world/quests.md §1.7, §6.2, §7.3, §9.1; specs/world/quests-helpers.md §5, §6; specs/world/quests-act1.md §10.2; specs/world/cube.md §1, §2; specs/world/waypoints.md §6; specs/world/hirelings.md §6 r1, §8 r1, §11; specs/world/hirelings-2.md §15, §17, §19
//! [`WiredWorld`]: the wired single-player host. The NPC, vendor, quest
//! and cube systems on their `d2-sim` providers
//! (`d2_sim::wiring::interaction`: [`Desk`] for `NpcWorld +
//! NpcVendors`, [`VendorDesk`] for `VendorWorld`;
//! `d2_sim::wiring::economy`: [`EconomyQuests`] for `QuestWorld`, on the
//! same desk; `EconomyCube` for the cube, `handlers::items`), beside the
//! waypoints and skill handlers of [`ActionWorld`].
//!
//! One unit world: the economy ([`Economy`]) is built per call from the
//! action wiring's own unit records, stat lists, unit data and hooks
//! (`ActionSim::sys`), so the player, the NPCs, the store items, the
//! cube's items and the monsters' drops are the same units the rest of
//! the game sees, in the game's one item store (`ActionHooks::items`,
//! lent to the economy for the call).
//!
//! One inventory per unit: the inventory model of the item moves
//! ([`WiredWorld::inventory`], `d2_sim::wiring::inventory`) is also the
//! vendors' ([`InvVendors`]: ownership, cursor, placement, removal of the
//! player's items) and the cube's (item list, checks, placement,
//! removal), so an item placed by C→S 0x18 can be sold and cubed, and an
//! item bought or transmuted lands where the moves see it.
//!
//! One home per game field: the game seed and the creation fields
//! (difficulty, expansion, game type, ladder; the item format follows
//! from the expansion) live on the action wiring (`ActionHooks::game_seed`,
//! `ActionHooks::ai_info`, `UnitData::expansion`, written at game
//! creation by
//! [`super::ActionEvents::create_game`]); the economy's [`GameFields`]
//! are built from them for each call and the seed is written back
//! ([`WiredWorld::with_economy`]). The unique bits (+0x1B24) live there
//! too (`ActionHooks::uniques`): one store for the handlers' economy, the
//! lent quest parts and the object and monster drops of the action
//! wiring (`ActionHooks::object_drops`).
//!
//! One owner of the player's interaction (+0x64 GUID, +0x68 type, +0x6C
//! active): the player's unit record
//! (`d2_sim::units::record::InteractInfo`), which the NPC, vendor,
//! quest, cube, inventory, waypoint and object wirings read and set.
//!
//! What no written spec provides (player data, the item copy
//! `0x0055A2A0`, the item routines of `vendors.md` without a written body,
//! the transport of the rests' messages) is the rest `R` ([`TradeRest`]);
//! its messages leave through [`Outbox`].

use d2_sim::game::Game;
use d2_sim::items::ItemTables;
use d2_sim::units::{RoomId, UnitId};
use d2_sim::wiring::action::Pending;
use d2_sim::wiring::action::{ActionHooks, ObjectCase};
use d2_sim::wiring::economy::{
    quest_objects, Economy, EconomyQuests, GameFields, HostQuests, QuestInv, QuestInventory,
    QuestLoan, QuestRest,
};
use d2_sim::wiring::interaction::{
    Desk, InteractionError, InteractionState, NpcRest, PlayerQuestsRef, VendorDesk, VendorRest,
};
use d2_sim::world::hirelings::life;
use d2_sim::world::npc::NpcControl;
use d2_sim::world::quests::{HostRequest, QuestControl};
use d2_sim::world::vendors::{GlobalLists, VendorTables};
use d2_sim::world::waypoints::{
    ObjectFacts, PlayerFacts, RoomRect, WaypointRecords, WaypointWorld,
};

use super::super::items::moves::{take_sent as inv_take_sent, InvParts, MoveCall};
use super::super::items::{CubeCall, CubeParts, InvVendors};
use super::super::player::{self, HostFacts, Outcome as PlayerOutcome, Run as PlayerRun};
use super::super::skills::{Call as SkillCall, Handled as SkillHandled, NoSkills, SkillHost};
use super::super::walk::{WalkCall, WalkResult};
use super::{
    ActionEvents, ActionWorld, NpcCall, Outbox, QuestCall, VendorCall, WaypointCall, WorldFault,
    WorldHost,
};
use d2_sim::world::npc::NpcWorld;

/// The seams of the NPC and vendor wiring without a provider: the
/// interaction rests of `d2_sim::wiring::interaction`
/// (`docs/handoff/wire-interaction.md` §6 lists each call's owner) and
/// the outbox their messages go to (`QuestRest::send`, and
/// `VendorRest::send_transaction` as `d2_sim::world::npc::transaction`
/// bytes), in send order. Its `NpcRest` interaction calls are the
/// host's one owner of the player's interaction.
pub trait TradeRest: NpcRest + VendorRest + QuestRest + PlayerQuestsRef + Outbox {}

impl<R: NpcRest + VendorRest + QuestRest + PlayerQuestsRef + Outbox> TradeRest for R {}

/// The wired host of a game on `ActionSim` (or `WorldSim`): the action
/// systems ([`ActionWorld`], with the skill slot `S`), the economy's own
/// parts (item tables; the item store and the unique bits are the action
/// wiring's `ActionHooks::items`, `ActionHooks::uniques`), the cube's parts, the inventory
/// model, the quests, the NPC control block, the vendor tables and the
/// interaction state (one vendor record per NPC record, the NPCs'
/// interaction lists), and the rest.
pub struct WiredWorld<R, S = NoSkills> {
    /// Waypoints, arrivals, the skill slot and the handlers' faults.
    pub action: ActionWorld<S>,
    pub tables: ItemTables,
    /// The cube (`None`: 0x2A, 0x4F stay stubs).
    pub cube: Option<CubeParts>,
    /// The game's one inventory model and the item-move seams (`None`:
    /// the item-move ids stay stubs, `handlers::items::moves`; the
    /// vendors and the cube see empty inventories).
    pub inventory: Option<InvParts>,
    pub quests: QuestControl,
    pub npc: NpcControl,
    pub vendor_tables: VendorTables,
    pub state: InteractionState,
    pub rest: R,
    /// Host milliseconds (`GetTickCount`), an input of store generation
    /// and refresh (`vendors.md` edge case 10); the caller keeps it
    /// current.
    pub now: u32,
    /// What the inventory rules queued during vendor calls (receiving
    /// unit, bytes), sent after the rest's messages ([`WorldHost::take_sent`]).
    inv_sent: Vec<(UnitId, Vec<u8>)>,
}

impl<R, S> WiredWorld<R, S> {
    /// The vendor records at game creation (`npc.md` §1.1 step 5,
    /// `vendors.md` §1 rules 3–5) from the NPC records and the global
    /// column lists (`GlobalLists::build`). The creation fields are the
    /// action wiring's ([`super::ActionEvents::create_game`]).
    pub fn new(
        action: ActionWorld<S>,
        tables: ItemTables,
        quests: QuestControl,
        npc: NpcControl,
        vendor_tables: VendorTables,
        rest: R,
        now: u32,
    ) -> Self {
        let state = InteractionState::new(&npc, &GlobalLists::build(&vendor_tables));
        Self {
            action,
            tables,
            cube: None,
            inventory: None,
            quests,
            npc,
            vendor_tables,
            state,
            rest,
            now,
            inv_sent: Vec::new(),
        }
    }

    /// Runs `f` on the economy over the action wiring's unit side
    /// (units, stat lists, unit data, hooks, the game's item store, lent
    /// out of the hooks for the call) and this world's item parts, with
    /// the game fields built from their home (game seed, creation fields)
    /// and the seed, the store and the unique bits written back after the
    /// call. Item creation outside a handler (a fixture) goes through it;
    /// the inventory model is in [`Parts::inventory`] (`InvParts::desk`
    /// on the same economy).
    pub fn with_economy<D: ActionEvents, T>(
        &mut self,
        game: &mut Game,
        events: &mut D,
        f: impl FnOnce(&mut Economy<'_, ActionHooks<D::X>>, &mut Parts<'_, R>) -> T,
    ) -> T {
        let s = &mut events.action().sys;
        let mut fields = GameFields::from_action(
            s.hooks.game_seed,
            &s.hooks.ai_info,
            s.data.expansion,
            std::mem::take(&mut s.hooks.uniques),
        );
        let mut items = std::mem::take(&mut s.hooks.items);
        let out = {
            let mut econ = Economy {
                game,
                units: &mut s.units,
                stats: &mut s.stats,
                data: &s.data,
                hooks: &mut s.hooks,
                fields: &mut fields,
                tables: &self.tables,
                items: &mut items,
            };
            let mut parts = Parts {
                quests: &mut self.quests,
                npc: &mut self.npc,
                vendor_tables: &self.vendor_tables,
                state: &mut self.state,
                cube: self.cube.as_mut(),
                inventory: self.inventory.as_mut(),
                rest: &mut self.rest,
                now: self.now,
            };
            f(&mut econ, &mut parts)
        };
        s.hooks.items = items;
        s.hooks.game_seed = fields.seed;
        s.hooks.uniques = fields.uniques;
        out
    }

    /// Runs `f` on the desk over [`Self::with_economy`]'s economy and
    /// this world, with the NPC control block and the inventory model.
    pub(super) fn desk<D: ActionEvents, T>(
        &mut self,
        game: &mut Game,
        events: &mut D,
        f: impl FnOnce(
            &mut Desk<'_, '_, ActionHooks<D::X>, R>,
            &mut NpcControl,
            Option<&mut InvParts>,
        ) -> T,
    ) -> T {
        self.with_economy(game, events, |econ, p| {
            let mut desk = Desk {
                econ,
                quests: &mut *p.quests,
                vendor_tables: p.vendor_tables,
                state: &mut *p.state,
                rest: &mut *p.rest,
                now: p.now,
            };
            f(&mut desk, &mut *p.npc, p.inventory.as_deref_mut())
        })
    }
}

/// A quest call on the desk's economy and rest ([`HostQuests`]: the
/// [`EconomyQuests`] calls with the object, level, interaction and
/// identify calls answered by the action wiring and the NPC rest, the
/// NPCs' interaction lists of the interaction state and, when the host
/// has one, the inventory model, [`QuestInv`]: the reward `0x005466B0`,
/// `quests.md` §9.1, and the cube close): the
/// mercenary rewards `0x00579180` an Act I quest grants (`quests-act1.md`
/// §10.2) are queued during the call with the sends that follow them
/// ([`d2_sim::wiring::economy::QuestDeferred`]) and run on the NPC
/// control block right after it, then the queued sends
/// (`quests-act1-rest.md` §8 item 8). A reward's NPC error, and an item
/// creation error of the reward, go to the interaction state's errors.
/// Also returned: what the inventory model sent during the call
/// (receiving unit, bytes), for [`WorldHost::take_sent`]'s inventory part.
///
/// TODO(quests.md §9.1): the order of a reward's inventory messages
/// against the quest messages of the same call is not written; they
/// follow the rest's, as the vendor calls' do.
fn quest_call<X: Pending, R: TradeRest, T>(
    desk: &mut Desk<'_, '_, ActionHooks<X>, R>,
    ctl: &mut NpcControl,
    mut inv: Option<&mut InvParts>,
    f: impl FnOnce(&mut QuestControl, &mut HostQuests<'_, '_, X, R>) -> T,
) -> (T, Vec<(UnitId, Vec<u8>)>) {
    let mut deferred = Vec::new();
    let mut errors = Vec::new();
    let out = {
        let mut lent = inv
            .as_deref_mut()
            .map(|p| QuestInv::new(&p.tables, &mut p.state, p.rest.as_mut()));
        let out = {
            let mut inner = EconomyQuests::new(&mut *desk.econ, &mut *desk.rest);
            inner.deferred = Some(&mut deferred);
            let mut w = HostQuests::new(inner);
            w.chats = Some(&mut desk.state.lists);
            w.inventory = lent
                .as_mut()
                .map(|q| q as &mut dyn QuestInventory<ActionHooks<X>>);
            f(&mut *desk.quests, &mut w)
        };
        if let Some(q) = lent {
            errors = q.errors;
        }
        out
    };
    for e in errors {
        desk.state.errors.push(InteractionError::Economy(e));
    }
    let sent = match inv {
        Some(p) => {
            let mut d = p.desk(&mut *desk.econ);
            inv_take_sent(&mut d)
                .into_iter()
                .filter_map(|(u, b)| Some((u?, b)))
                .collect()
        }
        None => Vec::new(),
    };
    for d in deferred {
        let Some((p, class)) = d.run(&mut *desk.rest) else {
            continue;
        };
        if let Err(e) = ctl.quest_mercenary(desk, p, class) {
            desk.state.errors.push(InteractionError::Npc(e));
        }
    }
    (out, sent)
}

/// The object module's queued quest routes
/// (`ActionSim::route_quest_objects`) run on the quest control
/// (`d2_sim::wiring::economy::quest_objects`) until none is left; the
/// routes no quest spec states go to the action wiring's
/// `Pending::object_route`, as without the queue. Returns what the
/// inventory model sent ([`quest_call`]).
fn quest_objects<X: Pending, R: TradeRest>(
    desk: &mut Desk<'_, '_, ActionHooks<X>, R>,
    ctl: &mut NpcControl,
    mut inv: Option<&mut InvParts>,
) -> Vec<(UnitId, Vec<u8>)> {
    let mut sent = Vec::new();
    loop {
        let calls = desk
            .econ
            .hooks
            .objects
            .as_mut()
            .map(|s| s.take_quest_calls())
            .unwrap_or_default();
        if calls.is_empty() {
            return sent;
        }
        let (back, s) = quest_call(desk, ctl, inv.as_deref_mut(), |q, w| {
            quest_objects::run_all(q, w, calls)
        });
        sent.extend(s);
        for r in back {
            desk.econ.hooks.x.object_route(desk.econ.game, r);
        }
    }
}

impl<R: TradeRest + Default + 'static, S> WiredWorld<R, S> {
    /// Runs `f` with this world's quest parts (the quest control, the
    /// rest, the item tables) lent to the action hooks
    /// ([`QuestLoan`], `ActionHooks::quest_host`), so a quest init,
    /// operate or object event 7 the object module hands back during `f`
    /// runs at once, inside its allocation, dispatch or timer event
    /// (`quests-act1-rest.md` §9 item 7; `objects.md` §3 rule 6). The
    /// parts come back after `f`; `f` must not use them through `self`
    /// (it gets only the action world). Hooks that already hold a quest
    /// host keep it.
    fn lend_quests<D: ActionEvents, T>(
        &mut self,
        events: &mut D,
        f: impl FnOnce(&mut ActionWorld<S>, &mut D) -> T,
    ) -> T {
        if events.action().sys.hooks.quest_host.is_some() {
            return f(&mut self.action, events);
        }
        let empty = self.quests.emptied();
        let loan = QuestLoan {
            quests: std::mem::replace(&mut self.quests, empty),
            rest: std::mem::take(&mut self.rest),
            tables: std::mem::take(&mut self.tables),
        };
        events.action().sys.hooks.quest_host = Some(Box::new(loan));
        let out = f(&mut self.action, events);
        let back = events
            .action()
            .sys
            .hooks
            .quest_host
            .take()
            .map(|h| h.into_any().downcast::<QuestLoan<R>>());
        match back {
            Some(Ok(l)) => {
                let l = *l;
                self.quests = l.quests;
                self.rest = l.rest;
                self.tables = l.tables;
            }
            // `f` took the loan out of the hooks or put another one in:
            // the game's quest state is gone (API misuse, fatal).
            _ => panic!("WiredWorld::lend_quests: the lent quest parts did not come back"),
        }
        out
    }
}

impl<R: TradeRest, S> WiredWorld<R, S> {
    /// The pet follows `0x005754B0` the placements queued
    /// (`path-placement.md` §10 rule 6, `ActionHooks::pet_follows`, on
    /// from the first frame): `hirelings.md` §6 rule 1 on the hireling
    /// list ([`life::follow`]; the other pet types have no list in
    /// `d2-sim` yet, `sim/pets.md`). A game without hireling tables has
    /// no hireling: the queue is dropped.
    ///
    /// TODO(path-placement.md §10 r6): in 1.14d the follow runs inside the
    /// placement; here it runs when the handler or tick that placed the
    /// player returns (the hireling state is the host's).
    pub fn pet_follows<D: ActionEvents>(&mut self, game: &mut Game, events: &mut D) {
        let q = events
            .action()
            .sys
            .hooks
            .pet_follows
            .as_mut()
            .map(std::mem::take)
            .unwrap_or_default();
        if q.is_empty() || self.state.hireling_tables.is_none() {
            return;
        }
        self.desk(game, events, |desk, _, _| {
            desk.with_hirelings(|w, t, st| {
                for p in q {
                    life::follow(w, t, st, p);
                }
            })
        });
    }

    /// The hireling deaths the kill queued (`ActionHooks::pet_deaths`, on
    /// from the first frame): `hirelings.md` §8 rule 1 → `0x005751A0`
    /// ([`life::on_kill`] with flag 1) for each killed monster with a
    /// player owner and a hireling node; then the owners' deaths the
    /// player mode-17 start queued (`ActionHooks::owner_deaths`):
    /// `hirelings-2.md` §15 → `0x00575BC0` ([`life::player_death`], every
    /// game type). Without hireling tables the game has no hireling: the
    /// queues are dropped.
    ///
    /// TODO(hirelings.md §8 r1, hirelings-2.md §15 r5): in 1.14d
    /// `0x005751A0` runs inside the kill, before the killer bookkeeping
    /// and the death mode, and `0x00575BC0` inside the mode-17 start right
    /// after the corpse creation; here both run when the handler or tick
    /// that queued them returns (the hireling state is this host's, not
    /// the action wiring's), so their 0x9B / 0x7A follow the call's other
    /// messages, and a kill and an owner death queued in one call run
    /// kills first.
    pub fn pet_deaths<D: ActionEvents>(&mut self, game: &mut Game, events: &mut D) {
        let hooks = &mut events.action().sys.hooks;
        let q = hooks
            .pet_deaths
            .as_mut()
            .map(std::mem::take)
            .unwrap_or_default();
        let owners = hooks
            .owner_deaths
            .as_mut()
            .map(std::mem::take)
            .unwrap_or_default();
        if (q.is_empty() && owners.is_empty()) || self.state.hireling_tables.is_none() {
            return;
        }
        self.desk(game, events, |desk, _, _| {
            desk.with_hirelings(|w, _, st| {
                for m in q {
                    life::on_kill(w, st, m, true);
                }
                for p in owners {
                    life::player_death(w, st, p);
                }
            })
        });
    }
}

/// The parts of a [`WiredWorld`] beside the economy, borrowed for one
/// [`WiredWorld::with_economy`] call.
pub struct Parts<'p, R> {
    pub quests: &'p mut QuestControl,
    pub npc: &'p mut NpcControl,
    pub vendor_tables: &'p VendorTables,
    pub state: &'p mut InteractionState,
    pub cube: Option<&'p mut CubeParts>,
    /// The inventory model (`InvParts::desk` on the call's economy).
    pub inventory: Option<&'p mut InvParts>,
    pub rest: &'p mut R,
    pub now: u32,
}

/// The action wiring's [`WaypointWorld`] with the difficulty of the
/// game's home (`ActionHooks::ai_info`); every other call goes to the
/// action wiring.
pub struct HostWaypoints<'w, W> {
    pub inner: &'w mut W,
    pub difficulty: u8,
}

impl<W: WaypointWorld> WaypointWorld for HostWaypoints<'_, W> {
    fn frame(&self) -> i32 {
        self.inner.frame()
    }
    fn difficulty(&self) -> u8 {
        self.difficulty
    }
    fn records(&mut self, player: UnitId) -> Option<&mut WaypointRecords> {
        self.inner.records(player)
    }
    fn object(&self, guid: u32) -> Option<(UnitId, ObjectFacts)> {
        self.inner.object(guid)
    }
    fn player(&self, player: UnitId) -> PlayerFacts {
        self.inner.player(player)
    }
    fn room_rect(&self, room: RoomId) -> RoomRect {
        self.inner.room_rect(room)
    }
    fn set_object_mode(&mut self, object: UnitId, mode: u8) {
        self.inner.set_object_mode(object, mode);
    }
    fn schedule_endanim(&mut self, object: UnitId, frame: i32) {
        self.inner.schedule_endanim(object, frame);
    }
    fn player_busy(&self, player: UnitId) -> bool {
        self.inner.player_busy(player)
    }
    fn set_interact(&mut self, player: UnitId, unit_type: u8, guid: u32) {
        self.inner.set_interact(player, unit_type, guid);
    }
    fn reset_interact(&mut self, player: UnitId) {
        self.inner.reset_interact(player);
    }
    fn interact_guid(&self, player: UnitId) -> Option<u32> {
        self.inner.interact_guid(player)
    }
    fn hostile_delay(&self, player: UnitId) -> bool {
        self.inner.hostile_delay(player)
    }
    fn attach_sound(&mut self, player: UnitId, event: u8) {
        self.inner.attach_sound(player, event);
    }
    fn send(&mut self, player: UnitId, msg: &[u8]) {
        self.inner.send(player, msg);
    }
    fn warp(&mut self, player: UnitId, level: u32, tile_code: u8) {
        self.inner.warp(player, level, tile_code);
    }
    fn spawn_room(&mut self, level: u32, tile_code: u8) -> Option<RoomId> {
        self.inner.spawn_room(level, tile_code)
    }
    fn set_player_mode_arrival(&mut self, player: UnitId) {
        self.inner.set_player_mode_arrival(player);
    }
}

/// A waypoint call on the action wiring's view, wrapped by
/// [`HostWaypoints`].
struct HostWaypointRun<C> {
    call: C,
    difficulty: u8,
}

impl<C: WaypointCall> WaypointCall for HostWaypointRun<C> {
    type Out = C::Out;
    fn call<W: WaypointWorld>(
        self,
        data: &d2_sim::world::waypoints::WaypointData,
        arrivals: &mut d2_sim::world::waypoints::ArrivalList,
        w: &mut W,
    ) -> C::Out {
        let mut hw = HostWaypoints {
            inner: w,
            difficulty: self.difficulty,
        };
        self.call.call(data, arrivals, &mut hw)
    }
}

impl<D: ActionEvents, R: TradeRest + Default + 'static, S: SkillHost<D>> WorldHost<D>
    for WiredWorld<R, S>
where
    D::X: Outbox,
{
    fn npc<C: NpcCall>(&mut self, game: &mut Game, events: &mut D, call: C) -> Option<C::Out> {
        Some(self.desk(game, events, |desk, ctl, _| call.call(ctl, desk)))
    }

    /// The vendor records are lent out of the interaction state for the
    /// call (the module holds them while it calls the world, as
    /// `VendorDesk`'s own entry points do); the world is [`VendorDesk`]
    /// with the NPC control block (`NpcLink`), its player inventories
    /// answered by the inventory model ([`InvVendors`]).
    fn vendors<C: VendorCall>(
        &mut self,
        game: &mut Game,
        events: &mut D,
        call: C,
    ) -> Option<C::Out> {
        let (out, sent) = self.desk(game, events, |desk, ctl, inv| {
            let mut records = std::mem::take(&mut desk.state.vendors);
            let tables = desk.vendor_tables;
            let inner: VendorDesk<'_, '_, '_, _, _> = desk.vendors(Some(ctl));
            let mut w = InvVendors::new(inner, inv);
            let out = call.call(tables, &mut records, &mut w);
            let sent = std::mem::take(&mut w.sent);
            drop(w);
            desk.state.vendors = records;
            (out, sent)
        });
        self.inv_sent.extend(sent);
        Some(out)
    }

    /// The action wiring's waypoints, with the game's difficulty
    /// ([`HostWaypoints`]).
    fn waypoints<C: WaypointCall>(
        &mut self,
        game: &mut Game,
        events: &mut D,
        call: C,
    ) -> Option<C::Out> {
        let difficulty = events.action().hooks().ai_info.difficulty;
        let run = HostWaypointRun { call, difficulty };
        let out = WorldHost::<D>::waypoints(&mut self.action, game, events, run);
        self.pet_deaths(game, events);
        self.hireling_calls(game, events);
        self.pet_follows(game, events);
        out
    }

    /// The action wiring's 0x13 object case ([`ActionWorld`]); a quest
    /// operate it queued runs right after the dispatch, on this world's
    /// quest control ([`quest_objects`]: nothing follows the operate in
    /// the handler, `waypoints.md` §5.2).
    fn objects(
        &mut self,
        game: &mut Game,
        events: &mut D,
        player: UnitId,
        guid: u32,
    ) -> Option<ObjectCase> {
        let out = self.lend_quests(events, |a, ev| {
            WorldHost::<D>::objects(a, game, ev, player, guid)
        });
        let sent = self.desk(game, events, quest_objects);
        self.inv_sent.extend(sent);
        out
    }

    /// The tick with this world's quest parts lent to the action hooks
    /// ([`WiredWorld::lend_quests`]): quest object inits run inside their
    /// allocation and object event 7 inside its timer event, in the tick
    /// that runs them (`quests-act1-rest.md` §9 item 7; `tick.md` §3).
    fn run_tick(&mut self, game: &mut Game, events: &mut D)
    where
        D: d2_sim::tick::EventDispatch + d2_sim::tick::TickHooks,
    {
        self.lend_quests(events, |_, ev| d2_sim::tick::tick(game, ev));
    }

    /// The quest routes queued outside a lent call (a quest call's own
    /// allocations, [`quest_objects`]), before the tick's sends are taken.
    fn after_tick(&mut self, game: &mut Game, events: &mut D) {
        let sent = self.desk(game, events, quest_objects);
        self.inv_sent.extend(sent);
        self.pet_deaths(game, events);
        self.hireling_calls(game, events);
        self.pet_follows(game, events);
        self.drive_hirelings(game, events);
    }

    /// The quest control on the desk's economy and rest
    /// ([`EconomyQuests`]). The mercenary rewards `0x00579180` an Act I
    /// quest grants (`quests-act1.md` §10.2) are queued during the call, with
    /// the sends that follow them
    /// ([`d2_sim::wiring::economy::QuestDeferred`]), and run on the NPC
    /// control block right after it (`npc.md` §7.5,
    /// [`d2_sim::world::npc::NpcControl::quest_mercenary`] with the desk
    /// as its world), then the queued sends, before the result is
    /// returned: the reward's 0x50 precedes the text refresh's 0x27 / 0x29
    /// (`quests-act1-rest.md` §8 item 8). The order of
    /// `Desk::quest_message`, here for every quest call. A reward's NPC
    /// error goes to the interaction state's errors, as there.
    ///
    /// The object module's queued quest routes run before the call and
    /// after it ([`quest_objects`]).
    fn quests<C: QuestCall>(&mut self, game: &mut Game, events: &mut D, call: C) -> Option<C::Out> {
        let (out, sent) = self.desk(game, events, |desk, ctl, mut inv| {
            let mut sent = quest_objects(desk, ctl, inv.as_deref_mut());
            let (out, s) = quest_call(desk, ctl, inv.as_deref_mut(), |q, w| call.call(q, w));
            sent.extend(s);
            sent.extend(quest_objects(desk, ctl, inv));
            (out, sent)
        });
        self.inv_sent.extend(sent);
        Some(out)
    }

    /// The cube on this world's economy and inventory model.
    fn cube<C: CubeCall>(&mut self, game: &mut Game, events: &mut D, call: C) -> Option<C::Out> {
        self.cube.as_ref()?;
        Some(self.with_economy(game, events, |econ, p| {
            let parts = p.cube.as_deref_mut().expect("checked above");
            let inv = p.inventory.as_deref_mut();
            call.call(econ, parts, inv)
        }))
    }

    /// The item moves on this world's economy and inventory parts (lent
    /// out of the world for the call).
    fn moves<C: MoveCall>(&mut self, game: &mut Game, events: &mut D, call: C) -> Option<C::Out> {
        let mut inv = self.inventory.take()?;
        // The hireling lists lent for the 0x61 give's hireling, owner
        // test and swap (`d2_sim::wiring::inventory::merc`), read only.
        inv.state.hirelings = self
            .state
            .hireling_tables
            .is_some()
            .then(|| self.state.hirelings.clone());
        let out = self.with_economy(game, events, |econ, _| call.call(econ, &mut inv));
        inv.state.hirelings = None;
        self.inventory = Some(inv);
        Some(out)
    }

    /// The skill handlers, then the pet follows their placements queued
    /// ([`WiredWorld::pet_follows`]).
    fn skill(&mut self, call: SkillCall<'_, D>) -> Option<SkillHandled> {
        let SkillCall {
            game,
            events,
            client,
            msg,
            staged,
        } = call;
        let call = SkillCall {
            game: &mut *game,
            events: &mut *events,
            client,
            msg,
            staged,
        };
        let out = WorldHost::<D>::skill(&mut self.action, call);
        self.pet_deaths(game, events);
        self.hireling_calls(game, events);
        self.pet_follows(game, events);
        out
    }

    /// The action wiring's provider with this host's answer, for 0x46 /
    /// 0x47, of the hireling list
    /// (`0x00574EC0(game, player, 7, 0)`, `NpcWorld::pet` on the desk);
    /// then the pet follows a warp queued ([`WiredWorld::pet_follows`]).
    fn player(
        &mut self,
        game: &mut Game,
        events: &mut D,
        run: PlayerRun<'_>,
    ) -> Option<PlayerOutcome> {
        let p = run.player;
        let hireling = matches!(run.msg.first(), Some(0x46 | 0x47)).then(|| {
            self.desk(game, events, |desk, _, _| {
                NpcWorld::pet(desk, p, d2_sim::world::hirelings::PET_HIRELING, 0)
            })
        });
        let facts = HostFacts { hireling };
        let out = player::action::run(game, events, &run, facts);
        self.hireling_calls(game, events);
        self.pet_follows(game, events);
        Some(out)
    }

    fn walk(&mut self, game: &mut Game, events: &mut D, call: WalkCall) -> Option<WalkResult> {
        let out = self.lend_quests(events, |a, ev| WorldHost::<D>::walk(a, game, ev, call));
        self.pet_deaths(game, events);
        self.hireling_calls(game, events);
        self.pet_follows(game, events);
        out
    }

    fn live_facts(
        &mut self,
        game: &Game,
        events: &mut D,
        unit: UnitId,
    ) -> Option<crate::adapters::UnitFacts> {
        WorldHost::<D>::live_facts(&mut self.action, game, events, unit)
    }

    fn vitals_sync(
        &mut self,
        game: &mut Game,
        events: &mut D,
        client: d2_sim::units::ClientId,
        staged: (u16, u16),
        queued: bool,
    ) -> Option<Vec<Vec<u8>>> {
        WorldHost::<D>::vitals_sync(&mut self.action, game, events, client, staged, queued)
    }

    /// The action wiring's sends (waypoints, tick paths), then what the
    /// inventory rules queued in vendor calls, then the rest's (NPC,
    /// vendor and quest messages); one system runs per message, so the
    /// systems never interleave. A vendor call's inventory messages (a
    /// targeting reset's 0x3F, placement and 0x9D sends) come before its
    /// 0x2A, the last call of each buy pass, sell or repair (`vendors.md`
    /// §7 "Message order").
    fn take_sent(&mut self, events: &mut D) -> Vec<(UnitId, Vec<u8>)> {
        let mut sent = events.action().hooks().x.take_sent();
        sent.append(&mut self.inv_sent);
        sent.extend(self.rest.take_sent());
        sent
    }

    /// The action wiring's object host tick, and [`WiredWorld::now`] (the
    /// vendors' clock): both read the host's millisecond clock
    /// (`GetTickCount`, `vendors.md` edge case 10), once per host frame.
    ///
    /// This host holds the quest control, so the object module's quest
    /// routes are queued for it from here on
    /// (`ActionSim::route_quest_objects`; the game's creator turns it on
    /// at creation, before the first object).
    fn host_tick(&mut self, events: &mut D, ms: u32) {
        events.action().route_quest_objects();
        let h = &mut events.action().sys.hooks;
        h.pet_follows.get_or_insert_with(Vec::new);
        h.pet_deaths.get_or_insert_with(Vec::new);
        h.owner_deaths.get_or_insert_with(Vec::new);
        h.hireling_calls.get_or_insert_with(Vec::new);
        WorldHost::<D>::host_tick(&mut self.action, events, ms);
        self.now = ms;
    }

    /// The host calls the quest rules raised since the last take
    /// (`quests-helpers.md` §6: game end, save pass), in call order,
    /// from the quest control (a lent control's come back with it,
    /// [`WiredWorld::lend_quests`]).
    fn take_host_requests(&mut self) -> Vec<HostRequest> {
        self.quests.take_host_requests()
    }

    fn fault(&mut self, fault: WorldFault) {
        self.action.faults.push(fault);
    }
}
