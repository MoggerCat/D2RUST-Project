// Spec: specs/world/npc.md §1.1, §2–§4; specs/world/vendors.md §1, §3, §4, §7
//! [`TradeWorld`]: the NPC and vendor systems on their `d2-sim`
//! providers (`d2_sim::wiring::interaction`: [`Desk`] for `NpcWorld +
//! NpcVendors`, [`VendorDesk`] for `VendorWorld`), beside the waypoints
//! of [`ActionWorld`].
//!
//! One unit world: the economy ([`Economy`]) is built per call from the
//! action wiring's own unit records, stat lists, unit data and hooks
//! (`ActionSim::sys`), so the player, the NPCs and the store items are
//! the same units the rest of the game sees. The game seed has one
//! owner, `ActionHooks::game_seed`: it is lent to [`GameFields::seed`]
//! for the call and written back after it.
//!
//! What no written spec provides (inventories, player data, the item
//! copy `0x0055A2A0`, the transport of the rests' messages) is the rest
//! `R` ([`TradeRest`]); its messages leave through [`Outbox`].

use d2_sim::game::Game;
use d2_sim::items::ItemTables;
use d2_sim::units::UnitId;
use d2_sim::wiring::action::ActionHooks;
use d2_sim::wiring::economy::{Economy, GameFields, ItemStore, QuestRest};
use d2_sim::wiring::interaction::{
    Desk, InteractionState, NpcRest, PlayerQuestsRef, VendorDesk, VendorRest,
};
use d2_sim::world::npc::NpcControl;
use d2_sim::world::quests::QuestControl;
use d2_sim::world::vendors::{GlobalLists, VendorTables};

use super::{
    ActionEvents, ActionWorld, NpcCall, Outbox, VendorCall, WaypointCall, WorldFault, WorldHost,
};

/// The seams of the NPC and vendor wiring without a provider: the
/// interaction rests of `d2_sim::wiring::interaction`
/// (`docs/handoff/wire-interaction.md` §6 lists each call's owner) and
/// the outbox their messages go to (`QuestRest::send`, and
/// `VendorRest::send_transaction` as `d2_sim::world::npc::transaction`
/// bytes), in send order.
pub trait TradeRest: NpcRest + VendorRest + QuestRest + PlayerQuestsRef + Outbox {}

impl<R: NpcRest + VendorRest + QuestRest + PlayerQuestsRef + Outbox> TradeRest for R {}

/// The world state of a game wired on `ActionSim` with NPCs and vendors:
/// the waypoint part ([`ActionWorld`]), the economy's own parts (game
/// fields, item tables, item store), the quests, the NPC control block,
/// the vendor tables and the interaction state (one vendor record per
/// NPC record, the NPCs' interaction lists), and the rest.
pub struct TradeWorld<R> {
    /// Waypoints, arrivals and the handlers' faults.
    pub action: ActionWorld,
    /// Game-creation fields; `seed` is a lent copy of
    /// `ActionHooks::game_seed` during a call (stale between calls).
    pub fields: GameFields,
    pub tables: ItemTables,
    pub items: ItemStore,
    pub quests: QuestControl,
    pub npc: NpcControl,
    pub vendor_tables: VendorTables,
    pub state: InteractionState,
    pub rest: R,
    /// Host milliseconds (`GetTickCount`), an input of store generation
    /// and refresh (`vendors.md` edge case 10); the caller keeps it
    /// current.
    pub now: u32,
}

impl<R> TradeWorld<R> {
    /// The vendor records at game creation (`npc.md` §1.1 step 5,
    /// `vendors.md` §1 rules 3–5) from the NPC records and the global
    /// column lists (`GlobalLists::build`).
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        action: ActionWorld,
        fields: GameFields,
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
            fields,
            tables,
            items: ItemStore::new(),
            quests,
            npc,
            vendor_tables,
            state,
            rest,
            now,
        }
    }

    /// Runs `f` on the economy over the action wiring's unit side
    /// (units, stat lists, unit data, hooks) and this world's item parts,
    /// with the game seed lent for the call. Item creation outside a
    /// handler (a fixture, a later drop path) goes through it.
    pub fn with_economy<D: ActionEvents, T>(
        &mut self,
        game: &mut Game,
        events: &mut D,
        f: impl FnOnce(&mut Economy<'_, ActionHooks<D::X>>, &mut Parts<'_, R>) -> T,
    ) -> T {
        let s = &mut events.action().sys;
        self.fields.seed = s.hooks.game_seed;
        let mut econ = Economy {
            game,
            units: &mut s.units,
            stats: &mut s.stats,
            data: &s.data,
            hooks: &mut s.hooks,
            fields: &mut self.fields,
            tables: &self.tables,
            items: &mut self.items,
        };
        let mut parts = Parts {
            quests: &mut self.quests,
            npc: &mut self.npc,
            vendor_tables: &self.vendor_tables,
            state: &mut self.state,
            rest: &mut self.rest,
            now: self.now,
        };
        let out = f(&mut econ, &mut parts);
        s.hooks.game_seed = self.fields.seed;
        out
    }

    /// Runs `f` on the desk over [`Self::with_economy`]'s economy and
    /// this world, with the NPC control block.
    fn desk<D: ActionEvents, T>(
        &mut self,
        game: &mut Game,
        events: &mut D,
        f: impl FnOnce(&mut Desk<'_, '_, ActionHooks<D::X>, R>, &mut NpcControl) -> T,
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
            f(&mut desk, &mut *p.npc)
        })
    }
}

/// The parts of a [`TradeWorld`] beside the economy, borrowed for one
/// [`TradeWorld::with_economy`] call.
pub struct Parts<'p, R> {
    pub quests: &'p mut QuestControl,
    pub npc: &'p mut NpcControl,
    pub vendor_tables: &'p VendorTables,
    pub state: &'p mut InteractionState,
    pub rest: &'p mut R,
    pub now: u32,
}

impl<D: ActionEvents, R: TradeRest> WorldHost<D> for TradeWorld<R>
where
    D::X: Outbox,
{
    fn npc<C: NpcCall>(&mut self, game: &mut Game, events: &mut D, call: C) -> Option<C::Out> {
        Some(self.desk(game, events, |desk, ctl| call.call(ctl, desk)))
    }

    /// The vendor records are lent out of the interaction state for the
    /// call (the module holds them while it calls the world, as
    /// `VendorDesk`'s own entry points do); the world is [`VendorDesk`]
    /// with the NPC control block (`NpcLink`).
    fn vendors<C: VendorCall>(
        &mut self,
        game: &mut Game,
        events: &mut D,
        call: C,
    ) -> Option<C::Out> {
        Some(self.desk(game, events, |desk, ctl| {
            let mut records = std::mem::take(&mut desk.state.vendors);
            let tables = desk.vendor_tables;
            let mut w: VendorDesk<'_, '_, '_, _, _> = desk.vendors(Some(ctl));
            let out = call.call(tables, &mut records, &mut w);
            desk.state.vendors = records;
            out
        }))
    }

    fn waypoints<C: WaypointCall>(
        &mut self,
        game: &mut Game,
        events: &mut D,
        call: C,
    ) -> Option<C::Out> {
        WorldHost::<D>::waypoints(&mut self.action, game, events, call)
    }

    /// The action wiring's sends (waypoints), then the rest's (NPC and
    /// vendor messages); one system runs per message, so the two never
    /// interleave.
    fn take_sent(&mut self, events: &mut D) -> Vec<(UnitId, Vec<u8>)> {
        let mut sent = events.action().hooks().x.take_sent();
        sent.extend(self.rest.take_sent());
        sent
    }

    fn fault(&mut self, fault: WorldFault) {
        self.action.faults.push(fault);
    }
}
