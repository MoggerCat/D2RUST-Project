// Spec: specs/world/object-functions.tsv; specs/world/objects.md §3, §7.2; specs/world/quests.md §9.5; specs/world/quests-act1-rest.md §1–§3, §9; specs/world/quests-act2.md §1.5; specs/world/quests-act2-2.md §2, §3
//! The quest routes of the object module ([`QuestObjectCall`], queued by
//! the action wiring when the host holds the quest control,
//! `ObjectState::route_quests`) run on the quest control: the init and
//! operate functions `object-functions.tsv` gives to `world/quests.md`
//! whose bodies the quest specs state, by function index, and object
//! event 7 through `0x005449E0` ([`quests::object_event`]). A route
//! whose function no quest spec states is handed back for the host's
//! `Pending::object_route` (as before the routing).
//!
//! Status: wired, unverified (no recording of a quest object yet).
//!
//! TODO(objects.md §3, §7.2): in 1.14d the quest init runs inside the
//! object's allocation and the operate inside the dispatch. The quest
//! control lives with the host, outside the action wiring's call stack,
//! so the queue runs when the host drains it: right after the dispatch
//! for C→S 0x13 (nothing follows the operate there, `waypoints.md` §5.2),
//! after the timer event that raised an object event 7, and after the
//! room-pass hook or tick that allocated the object for an init. An
//! init's draws and allocations (the marker init's town Cain) therefore
//! come after the rest of that hook's allocations; a recording of a
//! quest object's creation decides whether that order shows (HANDOFF §5).

use crate::game::Game;
use crate::items::ItemTables;
use crate::units::UnitId;
use crate::wiring::action::{ObjectRoute, Pending, QuestObjectCall, QuestObjectHost, View};
use crate::wiring::interaction::NpcRest;

use super::quest_reward::QuestInventory;
use super::{Economy, EconomyQuests, GameFields, HostQuests, QuestRest};
use crate::wiring::action::ActionHooks;
use crate::world::objects::{Dispatch, EventRun, Operate, Route};
use crate::world::quests::act3::{self, InitPoint, KhalimChest};
use crate::world::quests::{self, act1, act2, act4, act5, QuestControl, QuestWorld};

/// What running one queued route did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuestObjectRun {
    /// A quest function ran (or the spec states the function does
    /// nothing).
    Ran,
    /// The function's body is not stated by a quest spec, or only a part
    /// of it is (operate 34): hand the route to `Pending::object_route`.
    HandBack(ObjectRoute),
}

/// The host's quest parts lent to the action hooks for a call
/// ([`QuestObjectHost`], `ActionHooks::quest_host`): the quest control,
/// the quests' rest and the item tables (the unique bits are the hooks'
/// own, `ActionHooks::uniques`). A route runs on
/// [`HostQuests`] over an economy built from the call's view (its item
/// store, game seed and unique bits lent and written back, as
/// `with_economy` does),
/// without the deferred mercenary rewards (no object quest function
/// grants one).
pub struct QuestLoan<R, I = NoInventory> {
    pub quests: QuestControl,
    pub rest: R,
    pub tables: ItemTables,
    /// The host's inventory model, lent with the rest (the quest items:
    /// `has_item`, `delete_item`, the reward); none: the rest's answers.
    pub inv: Option<I>,
}

/// An inventory model a host lends to [`QuestLoan`]: `lend` runs `f` on
/// it as the quest calls' inventory ([`QuestInventory`]).
pub trait LoanedInventory {
    fn lend<X: Pending, T>(
        &mut self,
        f: impl FnOnce(&mut dyn QuestInventory<ActionHooks<X>>) -> T,
    ) -> T;
}

/// The loan with no inventory model (no value of this type exists).
pub enum NoInventory {}

impl LoanedInventory for NoInventory {
    fn lend<X: Pending, T>(
        &mut self,
        _: impl FnOnce(&mut dyn QuestInventory<ActionHooks<X>>) -> T,
    ) -> T {
        match *self {}
    }
}

impl<X: Pending, R: QuestRest + NpcRest + 'static, I: LoanedInventory + 'static> QuestObjectHost<X>
    for QuestLoan<R, I>
{
    fn run(
        &mut self,
        game: &mut Game,
        v: &mut View<'_, X>,
        call: QuestObjectCall,
    ) -> Option<ObjectRoute> {
        match self.on_world(game, v, LoanCall::Route(&call)) {
            LoanOut::Run(QuestObjectRun::HandBack(r)) => Some(r),
            _ => None,
        }
    }

    fn changed_level(
        &mut self,
        game: &mut Game,
        v: &mut View<'_, X>,
        player: UnitId,
        from: u32,
        to: u32,
    ) {
        self.on_world(game, v, LoanCall::ChangedLevel { player, from, to });
    }

    fn npc_wants_interact(
        &mut self,
        game: &mut Game,
        v: &mut View<'_, X>,
        player: UnitId,
        npc: UnitId,
        class: u16,
        interact: bool,
    ) -> bool {
        let call = LoanCall::NpcWantsInteract {
            player,
            npc,
            class,
            interact,
        };
        self.on_world(game, v, call) == LoanOut::Active(true)
    }

    fn map_ai_store(
        &mut self,
        game: &mut Game,
        v: &mut View<'_, X>,
        class: u16,
        path: Vec<(u32, i32, i32)>,
    ) {
        let call = LoanCall::MapAiStore { class, path: &path };
        self.on_world(game, v, call);
    }

    fn into_any(self: Box<Self>) -> Box<dyn std::any::Any> {
        self
    }
}

impl<R: QuestRest + NpcRest + 'static, I: LoanedInventory + 'static> QuestLoan<R, I> {
    /// Runs `call` on the quest control and a [`HostQuests`] over the
    /// game's parts (the economy of `v`, this loan's rest and tables),
    /// with the lent inventory model when there is one.
    fn on_world<X: Pending>(
        &mut self,
        game: &mut Game,
        v: &mut View<'_, X>,
        call: LoanCall<'_>,
    ) -> LoanOut {
        let h = &mut *v.h;
        let mut fields = GameFields::from_action(
            h.game_seed,
            &h.ai_info,
            v.data.expansion,
            std::mem::take(&mut h.uniques),
        );
        let mut items = std::mem::take(&mut h.items);
        let out = {
            let mut econ = Economy {
                game,
                units: &mut *v.units,
                stats: &mut *v.stats,
                data: v.data,
                hooks: &mut *v.h,
                fields: &mut fields,
                tables: &self.tables,
                items: &mut items,
            };
            let (quests, rest) = (&mut self.quests, &mut self.rest);
            match self.inv.as_mut() {
                Some(inv) => inv.lend::<X, _>(|q| on_host(&mut econ, rest, quests, Some(q), call)),
                None => on_host(&mut econ, rest, quests, None, call),
            }
        };
        v.h.items = items;
        v.h.game_seed = fields.seed;
        v.h.uniques = fields.uniques;
        out
    }
}

/// What a [`QuestLoan`] runs on the game's parts.
#[derive(Clone, Copy)]
enum LoanCall<'c> {
    /// One queued object route.
    Route(&'c QuestObjectCall),
    /// The map-AI store `0x00545C90` (`quests-act5.md` §5.8).
    MapAiStore {
        class: u16,
        path: &'c [(u32, i32, i32)],
    },
    /// Quest event 3 CHANGEDLEVEL (`world/quests.md` §4.1).
    ChangedLevel { player: UnitId, from: u32, to: u32 },
    /// The quest active test (`world/quests.md` §6.4).
    NpcWantsInteract {
        player: UnitId,
        npc: UnitId,
        class: u16,
        interact: bool,
    },
}

/// What a [`LoanCall`] gave back.
#[derive(Debug, PartialEq, Eq)]
enum LoanOut {
    Run(QuestObjectRun),
    /// The active test's answer.
    Active(bool),
}

/// `call` on [`HostQuests`] over `econ` and `rest`, with the host's
/// inventory model when one is lent.
fn on_host<'e, X: Pending, R: QuestRest>(
    econ: &'e mut Economy<'_, ActionHooks<X>>,
    rest: &'e mut R,
    quests: &mut QuestControl,
    inv: Option<&'e mut dyn QuestInventory<ActionHooks<X>>>,
    call: LoanCall<'_>,
) -> LoanOut {
    let inner = EconomyQuests::new(econ, rest);
    let mut w = HostQuests::new(inner);
    w.inventory = inv;
    match call {
        LoanCall::Route(c) => LoanOut::Run(run(quests, &mut w, c)),
        LoanCall::MapAiStore { class, path } => {
            let nodes: Vec<_> = path
                .iter()
                .map(|&(action, x, y)| crate::monsters::ai::MapNode {
                    action: action as i32,
                    x,
                    y,
                })
                .collect();
            // The copy's handle: index + 1 (0: none).
            w.inner.econ.hooks.map_ai_paths.push(nodes);
            let h = w.inner.econ.hooks.map_ai_paths.len() as u32;
            match class {
                543 => act5::q1::larzuk_map_ai(quests, &mut w, h),
                459 => act5::q3::map_ai_store(quests, &mut w, h, false),
                461 => act5::q3::map_ai_store(quests, &mut w, h, true),
                _ => {}
            }
            LoanOut::Run(QuestObjectRun::Ran)
        }
        LoanCall::ChangedLevel { player, from, to } => {
            quests.changed_level(&mut w, player, from, to);
            LoanOut::Run(QuestObjectRun::Ran)
        }
        // The set not picked (the fatal 0x7F4) answers false.
        LoanCall::NpcWantsInteract {
            player,
            npc,
            class,
            interact,
        } => LoanOut::Active(
            quests.npc_wants_interact(&mut w, player, npc, class, interact) == Ok(true),
        ),
    }
}

/// Runs the queued routes in order; returns those handed back.
pub fn run_all<W: QuestWorld>(
    ctl: &mut QuestControl,
    w: &mut W,
    calls: Vec<QuestObjectCall>,
) -> Vec<ObjectRoute> {
    calls
        .into_iter()
        .filter_map(|c| match run(ctl, w, &c) {
            QuestObjectRun::Ran => None,
            QuestObjectRun::HandBack(r) => Some(r),
        })
        .collect()
}

/// One queued route.
pub fn run<W: QuestWorld>(
    ctl: &mut QuestControl,
    w: &mut W,
    c: &QuestObjectCall,
) -> QuestObjectRun {
    match c.route {
        ObjectRoute::Init { object, created } if created.init == Route::Quest => {
            init(ctl, w, c, object, created.init_fn)
        }
        ObjectRoute::Operate(Dispatch::Quest(op)) => operate(ctl, w, c, op),
        ObjectRoute::Event {
            object,
            run: EventRun::Quest,
        } => {
            // `0x005449E0` (`quests.md` §9.5): by object class.
            quests::object_event(ctl, w, object, c.class);
            QuestObjectRun::Ran
        }
        r => QuestObjectRun::HandBack(r),
    }
}

/// The quest init functions a quest spec states, by `InitFn` index
/// (`object-functions.tsv` kind `init`, owner `world/quests.md`), with
/// the table address.
pub fn init_fn(n: u8) -> Option<u32> {
    Some(match n {
        // TowerTome (`quests-act1.md` §10.7).
        4 => 0x0059_5A00,
        // CairnStone, objects 17–21 (`quests-act1-rest.md` §2.2).
        6 => 0x0059_35E0,
        // CainGibbet → `0x00594060` (`quests-act1-rest.md` §9 item 8).
        7 => 0x0054_4990,
        // InifussTree (§9 item 9).
        9 => 0x0059_3FC0,
        // MalusStand (`quests-act1.md` §10.5).
        15 => 0x0054_4950,
        // JerhynPosition → `0x0059F380` (`quests-act2-2.md` §2).
        18 => 0x0054_48B0,
        // JerhynPositionEx → `0x0059F440` (`quests-act2-2.md` §2).
        19 => 0x0054_48E0,
        // TaintedAltar → `0x0059A3F0` (`quests-act2.md` §5.8).
        20 => 0x0054_4910,
        // HoradricOrifice (§8.8).
        21 => 0x0059_DB50,
        // ArcaneSanctuaryPortal (§6.9).
        29 => 0x0059_BA40,
        // HaremBlocker (§6.8).
        30 => 0x0059_B7D0,
        // HoradricChest 31–33: bare `ret` (`quests-act2.md` §1.5).
        31 => 0x0059_9EE0,
        32 => 0x0059_9F00,
        33 => 0x0059_9EF0,
        // TyraelsDoor (§8.8).
        38 => 0x0059_DAD0,
        // Act III (`quests-act3.md` §1.4, q-a3-quests).
        23 => 0x0054_4E30,
        25 => 0x005B_9AE0,
        39 => 0x005B_9D40,
        41 => 0x005B_8660,
        42 => 0x005B_86B0,
        43 => 0x005B_D1F0,
        44 => 0x005B_CBF0,
        45 => 0x005B_CB90,
        49 => 0x005B_70B0,
        50 => 0x005B_7160,
        52 => 0x005B_CE80,
        53 => 0x005B_BB70,
        60 => 0x005B_BBA0,
        // CountessChest (`quests-act1-rest.md` §4).
        47 => 0x0059_5A50,
        // CainStartPosition (`quests-act1-rest.md` §3).
        54 => 0x0059_40E0,
        // CainPortal (`quests-act1-rest.md` §9 item 10).
        61 => 0x0059_4290,
        // Act IV objects (`quests-act4.md` §1.4, q-a4-endgame).
        48 => 0x005B_5A20,
        55 => 0x005B_5590,
        56 => 0x005B_5620,
        59 => 0x0054_FE10,
        78 => 0x005B_5840,
        // Act V objects (`quests-act5.md` §1.4).
        62 => 0x0058_86A0,
        63 => 0x0058_D150,
        64 => 0x0058_D190,
        65 => 0x0058_D110,
        66 => 0x0058_EAC0,
        67 => 0x0058_A5B0,
        68 => 0x0058_A610,
        69 => 0x0058_A6C0,
        70 => 0x0058_7830,
        71 => 0x0058_7840,
        72 => 0x0058_D240,
        73 => 0x0058_D280,
        74 => 0x0058_AA50,
        75 => 0x0058_E670,
        76 => 0x0058_D640,
        77 => 0x0058_E710,
        79 => 0x0058_E830,
        _ => return None,
    })
}

/// The quest operate functions a quest spec states (in whole, or in
/// part for 34), by `OperateFn` index, with the table address.
pub fn operate_fn(n: u8) -> Option<u32> {
    Some(match n {
        // TowerTome (`quests-act1.md` §10.7).
        6 => 0x0059_4E70,
        // Act III (`quests-act3.md` §1.4, q-a3-quests).
        28 => 0x005B_7A60,
        31 => 0x005B_9B40,
        44 => 0x005B_84E0,
        45 => 0x005B_8530,
        53 => 0x005B_B980,
        57 => 0x005B_8860,
        58 => 0x005B_8940,
        59 => 0x005B_8A20,
        // Monolith (Cairn stone, `quests-act1.md` §10.6).
        9 => 0x0059_3710,
        // CainGibbet (`quests-act1-rest.md` §1.1).
        10 => 0x0059_3480,
        // InifussTree (`quests-act1.md` §10.6).
        12 => 0x0059_3AF0,
        // HoradrimMalus (`quests-act1.md` §10.5).
        21 => 0x0059_1AC0,
        // TaintedSunAltar (`quests-act2.md` §5.7).
        24 => 0x0059_A7E0,
        // StaffOrifice (§8.6).
        25 => 0x0059_DC70,
        // WirtsBody (`quests-act1-rest.md` §9 item 11).
        33 => 0x0058_3E70,
        // ArcaneSanctuaryPortal: its `0x0059BAF0(level)` call (§6.9).
        34 => 0x0058_46B0,
        // Cube / scroll / staff chests (§4.7).
        39 => 0x0059_9DF0,
        40 => 0x0059_9C10,
        41 => 0x0059_9CF0,
        // SanctuaryTome (§6.7).
        42 => 0x0059_B970,
        // Act IV objects (`quests-act4.md` §1.4, q-a4-endgame).
        49 => 0x005B_5C10,
        52 => 0x005B_5630,
        54 => 0x005B_6B70,
        55 => 0x005B_6BC0,
        56 => 0x005B_6C10,
        73 => 0x005B_5880,
        // Act V objects (`quests-act5.md` §1.4).
        62 => 0x0058_D1E0,
        63 => 0x0058_D200,
        64 => 0x0058_D220,
        65 => 0x0058_D310,
        66 => 0x0058_D400,
        67 => 0x0058_ABC0,
        69 => 0x0058_D5E0,
        70 => 0x0058_E6A0,
        71 => 0x0058_D6A0,
        72 => 0x0058_E740,
        _ => return None,
    })
}

/// Init function `n` ([`init_fn`]) on the object.
fn init<W: QuestWorld>(
    ctl: &mut QuestControl,
    w: &mut W,
    c: &QuestObjectCall,
    object: crate::units::UnitId,
    n: u8,
) -> QuestObjectRun {
    if init_fn(n).is_none() {
        return QuestObjectRun::HandBack(c.route);
    }
    // The init point of the object (room, x, y); a null room has none.
    let at = c.room.map(|room| InitPoint {
        room,
        x: c.x,
        y: c.y,
    });
    w.set_init_point(c.room.map(|room| (object, c.x, c.y, room)));
    match n {
        4 => act1::q5::object_init(ctl, w, object),
        23 => act3::tome_init(ctl, w, object),
        25 => {
            if let Some(at) = at {
                act3::decoy_init(ctl, w, object, at);
            }
        }
        39 => {
            if let Some(at) = at {
                act3::altar_init(ctl, w, object, at);
            }
        }
        41 => act3::stairs_init(ctl, w, object),
        42 => act3::lever_init(ctl, w, object),
        43 => {
            if let Some(at) = at {
                act3::wanderer_init(ctl, w, at);
            }
        }
        44 => act3::hellgate_init(ctl, w, object),
        45 => act3::bridge_init(ctl, w, object),
        49 => {
            if let Some(at) = at {
                act3::hratli_start_init(ctl, w, at);
            }
        }
        50 => {
            if let Some(at) = at {
                act3::hratli_end_init(ctl, w, at);
            }
        }
        52 => act3::natalya_init(ctl, w, object),
        53 => act3::stairs_r_init(ctl, w, object),
        60 => act3::orb_init(ctl, w, object),
        48 => act4::q3::forge_init(ctl, w, object),
        55 => act4::q2::start_point_init(ctl, w, object),
        59 => act4::q2::dummy_init(w, object),
        78 => act4::q2::portal_init(ctl, w, object),
        // The stone's class is its value (`quests-act1-rest.md` §2.1).
        6 => act1::q4::stone_init(ctl, w, object, c.class),
        7 => act1::q4::gibbet_init(ctl, w, object),
        9 => act1::q4::tree_init(ctl, w, object),
        15 => act1::malus_init(ctl, w, object),
        18 => {
            if let Some(at) = at {
                act2::q4::start_jerhyn_init(ctl, w, at);
            }
        }
        19 => {
            if let Some(at) = at {
                act2::q4::palace_jerhyn_init(ctl, w, object, at);
            }
        }
        20 => act2::q3::altar_init(ctl, w, object),
        21 => act2::q6::orifice_init(ctl, w, object),
        29 => act2::q4::portal_init(ctl, w, object),
        30 => act2::q4::blocker_init(ctl, w, object),
        38 => act2::q6::door_init(ctl, w, object),
        47 => act1::q5::chest_init(ctl, w, object),
        // The init args' room and position; a null room spawns nothing
        // (`quests-act1-rest.md` §9 item 2).
        54 => act1::q4::marker_init(ctl, w, object, c.room, c.x, c.y),
        61 => act1::q4::cain_portal_init(w, object),
        62 => act5::q2::cage_init(ctl, w, object),
        // The statue's class (474–476) picks its slot (part 2 §7.8).
        63..=65 => act5::q5::statue_init(ctl, w, object, c.class),
        66 => act5::q3::anya_town_dummy_init(ctl, w, object),
        67 => act5::q3::anya_dummy_init(ctl, w, object),
        68 => act5::q3::nihlathak_town_dummy_init(ctl, w, object),
        69 => act5::q3::nihlathak_temple_dummy_init(ctl, w, object),
        71 => act5::q1::larzuk_dummy_init(ctl, w, object),
        72 => act5::q5::altar_init(ctl, w, object),
        73 => act5::q5::keep_door_init(ctl, w, object),
        74 => act5::q3::frozen_anya_init(ctl, w, object),
        75 => act5::q6::portal_init(ctl, w, object),
        76 => act5::q5::summit_door_init(ctl, w, object),
        77 => act5::q6::last_portal_init(ctl, w, object),
        79 => act5::q6::zoo_init(ctl, w),
        // 31–33 and 70: `ret`.
        _ => {}
    }
    w.set_init_point(None);
    QuestObjectRun::Ran
}

/// Operate function `n` ([`operate_fn`]). Every quest operate the specs
/// state takes the operating player; without an operator the route is
/// handed back.
fn operate<W: QuestWorld>(
    ctl: &mut QuestControl,
    w: &mut W,
    c: &QuestObjectCall,
    op: Operate,
) -> QuestObjectRun {
    let (Some(player), Some(_)) = (op.operator, operate_fn(op.operate_fn)) else {
        return QuestObjectRun::HandBack(c.route);
    };
    let o = op.object;
    match op.operate_fn {
        6 => act1::q5::tome_operate(ctl, w, o, player),
        28 => act3::tome_operate(ctl, w, o, player),
        31 => act3::decoy_operate(ctl, w, o, player),
        44 => act3::stairs_operate(ctl, w, o, player),
        45 => act3::lever_operate(ctl, w, o, player),
        53 => act3::orb_operate(ctl, w, o, player),
        57 => act3::chest_operate(ctl, w, o, player, KhalimChest::Heart),
        58 => act3::chest_operate(ctl, w, o, player, KhalimChest::Eye),
        59 => act3::chest_operate(ctl, w, o, player, KhalimChest::Brain),
        // The stone's value is its class (`quests-act1-rest.md` §2.1).
        9 => act1::q4::stone_operate(ctl, w, o, player, op.class),
        10 => act1::q4::gibbet_operate(ctl, w, o, player),
        12 => act1::q4::tree_operate(ctl, w, o, player),
        21 => act1::malus_operate(ctl, w, o, player),
        33 => act1::q4::wirt_body_operate(w, o),
        24 => {
            act2::q3::altar_operate(ctl, w, o, player);
        }
        // With the "no chain 13 record → 0" step (`quests-act2-2.md` §3).
        25 => {
            act2::q6::orifice_operate_checked(ctl, w, o, player);
        }
        // Only the `0x0059BAF0(level)` call is stated; the rest of the
        // operate (the object spec's) is handed back.
        // Mode 0 (`0x005846B0`): mode 1 and the open event one frame
        // after the animation (no quest call); an open portal runs the
        // level-dependent call below.
        34 if w.object_mode(o) == 0 => {
            w.set_object_mode(o, 1);
            let at = w.frame() + (w.object_anim_length(o) >> 8) + 1;
            w.schedule_object_event(o, 1, at);
            return QuestObjectRun::Ran;
        }
        34 => {
            if let Some(level) = w.unit_level(o) {
                act2::q4::portal_operate(ctl, w, level);
            }
            return QuestObjectRun::HandBack(c.route);
        }
        39 => {
            act2::q2::cube_chest(ctl, w, o, player);
        }
        40 => {
            act2::q2::scroll_chest(ctl, w, o, player);
        }
        41 => {
            act2::q2::staff_chest(ctl, w, o, player);
        }
        42 => act2::q4::tome_operate(ctl, w, o, player),
        62..=64 => {
            act5::q5::statue_operate(w, player);
        }
        65 => {
            act5::q5::altar_operate(ctl, w, o, player);
        }
        66 => {
            act5::q5::keep_door_operate(ctl, w, o, player);
        }
        67 => {
            act5::q3::frozen_anya_operate(ctl, w, o, player);
        }
        69 => {
            act5::q5::invisible_ancient_operate(w, o, player);
        }
        70 => {
            act5::q6::portal_operate(ctl, w, player);
        }
        71 => {
            act5::q5::summit_door_operate(ctl, w, o, player);
        }
        72 => {
            act5::q6::last_portal_operate(ctl, w, player);
        }
        49 => act4::q3::forge_operate(ctl, w, o, player),
        52 => act4::q2::seal_operate(ctl, w, o, player, op.class),
        54 => act4::q2::infector_seal_operate(ctl, w, o, player, op.class),
        55 => act4::q2::de_seis_seal_operate(ctl, w, o, player, op.class),
        56 => act4::q2::vizier_seal_operate(ctl, w, o, player, op.class),
        73 => act4::q2::portal_operate(ctl, w, o, player),
        _ => return QuestObjectRun::HandBack(c.route),
    }
    QuestObjectRun::Ran
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::objects::{init_route, operate_route};

    const FUNCS_TSV: &str = include_str!("../../../../../specs/world/object-functions.tsv");

    /// Every stated index against its `object-functions.tsv` row: owner
    /// `world/quests.md`, the same address, a quest route in the object
    /// module; returns the mismatches.
    fn check(init: impl Fn(u8) -> Option<u32>, operate: impl Fn(u8) -> Option<u32>) -> Vec<String> {
        let rows: Vec<(String, u32, u32, String)> = FUNCS_TSV
            .lines()
            .skip(1)
            .map(|l| {
                let f: Vec<&str> = l.split('\t').collect();
                let addr = u32::from_str_radix(f[2].trim_start_matches("0x"), 16).unwrap_or(0);
                (
                    f[0].to_string(),
                    f[1].parse().unwrap(),
                    addr,
                    f[5].to_string(),
                )
            })
            .collect();
        let mut bad = Vec::new();
        for n in 0..=u8::MAX {
            for (kind, got, route) in [
                ("init", init(n), init_route(n)),
                ("operate", operate(n), operate_route(n)),
            ] {
                let Some(addr) = got else { continue };
                let row = rows.iter().find(|r| r.0 == kind && r.1 == u32::from(n));
                match row {
                    Some(r) if r.2 == addr && r.3 == "world/quests.md" && route == Route::Quest => {
                    }
                    _ => bad.push(format!("{kind} {n}")),
                }
            }
        }
        bad
    }

    // Covers: specs/world/quests-act2.md §1.5
    #[test]
    fn stated_functions_match_the_table() {
        assert_eq!(check(init_fn, operate_fn), Vec::<String>::new());
        // Every Act II row of §1.5 with an init or operate is stated.
        for n in [20, 21, 29, 30, 31, 32, 33, 38] {
            assert!(init_fn(n).is_some(), "init {n}");
        }
        for n in [24, 25, 34, 39, 40, 41, 42] {
            assert!(operate_fn(n).is_some(), "operate {n}");
        }
    }

    #[test]
    fn act_v_functions_are_stated() {
        for n in [
            62, 63, 64, 65, 66, 67, 68, 69, 70, 71, 72, 73, 74, 75, 76, 77, 79,
        ] {
            assert!(init_fn(n).is_some(), "init {n}");
        }
        for n in [62, 63, 64, 65, 66, 67, 69, 70, 71, 72] {
            assert!(operate_fn(n).is_some(), "operate {n}");
        }
    }

    // Covers: specs/world/quests-act1-rest.md §9 r8, §9 r9, §9 r10, §9 r11
    #[test]
    fn act1_answered_functions_are_stated() {
        // WW-6: the gibbet, tree and cain portal inits, Wirt's body's
        // operate; Wirt's body has no init and init 37 is not Act I's.
        for n in [7, 9, 61] {
            assert!(init_fn(n).is_some(), "init {n}");
        }
        assert!(operate_fn(33).is_some());
        assert!(init_fn(37).is_none());
    }

    // Covers: specs/world/quests-act3.md §1.4
    #[test]
    fn act3_functions_are_stated() {
        for n in [23, 25, 39, 41, 42, 43, 44, 45, 49, 50, 52, 53, 60] {
            assert!(init_fn(n).is_some(), "init {n}");
        }
        for n in [28, 31, 44, 45, 53, 57, 58, 59] {
            assert!(operate_fn(n).is_some(), "operate {n}");
        }
    }

    // Covers: specs/world/quests-act4.md §1.4
    #[test]
    fn act4_functions_are_stated() {
        for n in [48, 55, 56, 59, 78] {
            assert!(init_fn(n).is_some(), "init {n}");
        }
        for n in [49, 52, 54, 55, 56, 73] {
            assert!(operate_fn(n).is_some(), "operate {n}");
        }
    }

    // M08: a wrong address and a non-quest index are reported.
    #[test]
    fn table_check_catches_perturbations() {
        let wrong = |n: u8| {
            if n == 10 {
                Some(0x0059_3481)
            } else {
                operate_fn(n)
            }
        };
        assert_eq!(check(init_fn, wrong), vec!["operate 10".to_string()]);
        let not_quest = |n: u8| {
            if n == 1 {
                Some(0x0058_6410)
            } else {
                init_fn(n)
            }
        };
        assert_eq!(check(not_quest, operate_fn), vec!["init 1".to_string()]);
    }
}
