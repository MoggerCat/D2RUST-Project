// Spec: specs/sim/tick.md §3, §5.6; specs/sim/units.md §5 (wiring of the action seams)
//! The "action" seams wired to their providers: combat on the unit
//! records and stat lists ([`combat`]); missiles and monster AI on
//! combat, units (modes, timer events) and the DRLG (rooms, collision)
//! ([`missiles`], [`ai`]); DRLG room activation on the act room lists of
//! [`crate::units::UnitLists`] ([`rooms`]); waypoints on the DRLG levels
//! ([`waypoints`]); objects on units, timers and the DRLG ([`objects`]);
//! and one dispatcher the tick runs ([`dispatch`]).
//!
//! Ownership: [`ActionSim`] holds a [`crate::units::dispatch::UnitSystem`] (unit records, stat
//! lists, unit tables) whose hooks are [`ActionHooks`] (DRLG, missile and
//! AI state, tables, combat lists). Timer events go through the unit
//! dispatch of `units.md` §5 (the handler tables of `tick.md` §5.6, with
//! the monster freeze drop); its hooks run the missile class handler
//! (missile events), the AI think (monster type 2) and the type-10 reset.
//! Each seam call builds a short-lived [`View`] (or [`combat::CombatView`])
//! over the parts it needs.
//!
//! Calls with no provider yet go to [`Pending`] (defaults: nothing).
//! Nothing here decides game behaviour: every rule stays in its module;
//! an adapter only maps one seam call to the provider's call.

pub mod ai;
pub mod combat;
pub mod death;
pub mod dispatch;
pub mod hirelings;
pub mod inactive;
pub mod missiles;
pub mod monster_add;
pub mod monsters;
pub mod objects;
pub mod pending;
pub mod reaction;
pub mod rooms;
pub mod switch;
pub mod unit_update;
pub mod units;
pub mod vitals_sync;
pub mod warp_tile;
pub mod waypoints;

#[cfg(any(test, feature = "bench-fixtures"))]
#[cfg_attr(not(test), allow(unused, dead_code))]
pub(crate) mod tests;

use std::collections::BTreeMap;
use std::sync::Arc;

use d2_data::tables::{Levels, Missiles as MissileRow};
use d2_formats::animdata::AnimData;

use crate::combat::vitals::VitalsTables;
use crate::combat::{CombatEntry, CombatTables};
use crate::drlg::{DrlgData, DrlgError, Dungeon, LevelTypes, TileSource};
use crate::game::Game;
use crate::missiles::MissileStore;
use crate::monsters::ai::{AiStore, GameInfo};
use crate::rng::Seed;
use crate::skills::SkillTables;
use crate::stats::StatLists;
use crate::units::hooks::{Sim, UnitData};
use crate::units::modes::UnitError;
use crate::units::record::Units;
use crate::units::UnitId;
use crate::world::waypoints::WaypointRecords;

pub use dispatch::ActionSim;
pub use hirelings::HirelingCall;
pub use monsters::MonsterWorld;
pub use objects::{
    ObjectCase, ObjectReach, ObjectRoute, ObjectState, ObjectView, QuestObjectCall, QuestObjectHost,
};
pub use pending::{KillStep, NoPending, Pending, SkillEvent};

/// The tables the action modules read (typed `d2_data` records).
#[derive(Debug, Clone)]
pub struct ActionTables {
    /// `missiles.txt` rows.
    pub missiles: Vec<MissileRow>,
    pub skills: SkillTables,
    /// Also the monstats / monstats2 the AI reads.
    pub combat: CombatTables,
    /// `levels.txt` rows (AI).
    pub levels: Vec<Levels>,
    /// `Sk1mode..Sk8mode` per monstats row ([`crate::monsters::ai::skill_modes`]).
    pub skill_modes: Vec<[u8; 8]>,
}

/// The DRLG side of a game: the acts' DRLGs and their services.
pub struct DrlgWorld {
    pub dungeon: Dungeon,
    pub data: Arc<DrlgData>,
    pub tiles: Box<dyn TileSource>,
    pub types: Box<dyn LevelTypes>,
}

/// What went wrong in an adapter (fatal assertions of 1.14d, API misuse,
/// a provider's error), in order.
#[derive(Debug, PartialEq, Eq)]
pub enum WiringError {
    /// A store was needed while it was lent out (a missile or AI call
    /// re-entered its own module).
    Reentrant(&'static str),
    Drlg(DrlgError),
    Unit(UnitError),
    /// The AnimData name lookup failed (`animdata.md` §4: a name longer
    /// than 8 characters, fatal 0xD9 / 0xDA in 1.14d).
    AnimData(d2_formats::FormatError),
    /// A path-core fatal assert (`sim/path-placement.md` §2–§6).
    Path(crate::path::PathError),
    /// A walk fatal assert (`sim/pathing.md`).
    Walk(crate::path::walk::WalkError),
    /// A placement fatal assert (`sim/path-placement.md` §7–§12).
    Place(crate::path::place_seams::PlaceError),
    /// An object fatal assert (`world/objects.md`).
    Object(crate::world::objects::ObjectError),
    /// Room ready `0x0061A460` on a client without a room: fatal assert
    /// 0x3EF in 1.14d (`sim/tick.md` §6 rule 6).
    NoClientRoom(crate::units::ClientId),
    /// The monster mode message (`sim/intents-events.md` §7.4): a fatal
    /// assert or a message the spec gives no layout for.
    ModeMessage(unit_update::ModeMessageError),
    /// The lightning fan / ring with a progressive step ≤ 0
    /// (`skills/bodies-4.md` Edge case 1): an endless loop in 1.14d;
    /// d2rs stopped it (as `StatListError::EndlessExpiry`).
    EndlessProgressive {
        unit: UnitId,
        skill: i32,
        step: i32,
    },
}

/// The [`crate::units::hooks::UnitHooks`] of [`ActionSim`]'s unit system
/// and the state every action adapter shares.
pub struct ActionHooks<X> {
    pub tables: Arc<ActionTables>,
    pub drlg: DrlgWorld,
    /// Lent to the missile code during a missile call (`None` then).
    pub missiles: Option<MissileStore>,
    /// Lent to the AI code during a think (`None` then).
    pub ai: Option<AiStore>,
    /// Game fields the AI reads (game +0x6A, +0x74, +0x6D).
    pub ai_info: GameInfo,
    /// Combat lists (unit +0xAC, `damage.md` §3 step 3), first = newest.
    pub combat_lists: BTreeMap<UnitId, Vec<CombatEntry>>,
    /// The process-wide element hit-class byte `0x0088CAD0`.
    pub hit_class: u8,
    /// The game seed of `rng.md` §5.3 (unit allocation).
    pub game_seed: Seed,
    /// Game +0x1B24, the unique bits (`quality.md` §8.1): the game's one
    /// store, read and set by every item creation (the chest drop of
    /// [`Self::object_drops`], a monster drop, the economy of the host's
    /// handlers and the lent quest parts). Zero in a new game (`cube.md`
    /// Inputs).
    pub uniques: crate::items::UniqueBits,
    /// The game's one item store (the item data of every item unit:
    /// drops, stores, inventories, the cube; `crate::wiring::economy`).
    /// Lent to an economy for a call (empty then).
    pub items: crate::wiring::economy::ItemStore,
    /// Waypoint records per player (player data +0x1C, `waypoints.md` §2).
    pub waypoints: BTreeMap<UnitId, WaypointRecords>,
    /// The object control (game +0x10F0, `objects.md` §2), the object
    /// tables and the host tick ([`objects`]). `None` (the default): not
    /// created ([`ActionSim::create_objects`]); the object routes keep
    /// their [`Pending`] answers. Lent to an object call (`None` then).
    pub objects: Option<ObjectState>,
    /// The object state is lent out for a call.
    objects_out: bool,
    /// The drop state of the object code's chest drop `D(Q)`
    /// (`treasure.md` §4, [`crate::wiring::economy::object_chest_drop`];
    /// the `levels` rows are the object tables'). `None` (the default):
    /// no drop (`ChestWorld::chest_drop` answers none, as before the
    /// provider).
    pub object_drops: Option<Box<crate::wiring::economy::DeathDrops>>,
    /// Players whose pets follow them after a placement
    /// (`path-placement.md` §10 rule 6, `0x005754B0`), for the host that
    /// holds the pet lists (`hirelings.md` §6 rule 1). `None` (the
    /// default): the call does nothing (no pet list in the action
    /// wiring).
    pub pet_follows: Option<Vec<UnitId>>,
    /// Monsters killed by the kill `0x0057CCB0` with flag 1 (every
    /// caller but the expired-pet kill), for the host that holds the
    /// hireling lists: `hirelings.md` §8 rule 1 (`0x005751A0` when the
    /// owner is a player). `None` (the default): nothing is recorded.
    pub pet_deaths: Option<Vec<UnitId>>,
    /// Players whose mode-17 start `0x0057FCA0` ran (after the corpse
    /// creation), for the host that holds the hireling lists:
    /// `hirelings-2.md` §15 (`0x00575BC0`, the hireling dies with its
    /// owner, every game type). `None` (the default): nothing is
    /// recorded.
    pub owner_deaths: Option<Vec<UnitId>>,
    /// The hireling calls met in order (save restore, join follow, act
    /// change; [`HirelingCall`]), for the host that holds the hireling
    /// lists (`hirelings-2.md` §19). `None` (the default): nothing is
    /// recorded.
    pub hireling_calls: Option<Vec<HirelingCall>>,
    /// The loaded `AnimData.d2` (`formats/animdata.md`, parsed by
    /// `d2-formats`): the records `UnitHooks::anim_record` looks up by
    /// COF name. `None`: no record for any unit (as before the table is
    /// given).
    pub anim_data: Option<Arc<AnimData>>,
    /// `experience.txt` / `charstats.txt` of the experience on a kill
    /// (`combat/vitals.md` §4). `None`: no experience is given.
    pub vitals: Option<Arc<VitalsTables>>,
    /// The target of the monster mode change running now (the record
    /// argument of `0x005A7C20`, `units.md` §4.6); set by the kill's
    /// death mode change (`damage.md` §7.2). Also the killer of a
    /// player's DT start (`0x00580A70`'s unit target, [`death`]): the
    /// host that starts it sets it.
    pub mode_target: Option<UnitId>,
    /// The mode of the monster mode change running now (the record's
    /// mode, `units.md` §4.6), for the start functions that read it (the
    /// attack / skill start `0x005A75C0`, rule 7).
    pub monster_request: u32,
    /// The monster state (monster data, umods, monster init) lent by the
    /// host that owns it ([`monsters`]: `WorldSim` lends its world state
    /// around its timer events and tick hooks). `None`: the monster
    /// routes keep their [`Pending`] answers.
    pub monster_world: Option<Box<dyn MonsterWorld<X>>>,
    /// The monster world is taken out for a call.
    monster_world_out: bool,
    /// The quest control lent by the host that holds it
    /// ([`objects::QuestObjectHost`]): a quest init, operate or object
    /// event 7 the object module hands back runs on it at once, inside the
    /// allocation, dispatch or event (`quests-act1-rest.md` §9 item 7).
    /// `None`: queued for the host ([`ObjectState::route_quests`]) or
    /// handed to [`Pending::object_route`], as before.
    pub quest_host: Option<Box<dyn objects::QuestObjectHost<X>>>,
    /// The quest host is running a route: routes it raises are queued and
    /// run right after it.
    quest_host_out: bool,
    /// Objects allocated by [`View::allocate`] whose per-kind init waits
    /// for the allocation's game-seed step to be written back (`None`
    /// outside such an allocation).
    deferred_inits: Option<Vec<UnitId>>,
    /// Units between allocation steps 7 and 8 (`units.md` §3.1 r7.1),
    /// with the allocation's room argument (r7.2), innermost last.
    alloc_rooms: Vec<(UnitId, Option<crate::units::RoomId>)>,
    /// The unit path records and tables ([`crate::wiring::path`]).
    /// `None` (the default): the path seams keep their [`Pending`]
    /// answers; [`ActionHooks::enable_paths`] turns the provider on.
    pub paths: Option<Box<crate::wiring::path::PathState>>,
    /// The table data of the skill bodies (`skills/bodies.md`;
    /// [`crate::skills::use_::bodies::BodyTables`]). `None`: no
    /// itemstatcost flags, state groups or overlays (every lookup answers
    /// "no record").
    pub bodies: Option<Arc<crate::skills::use_::bodies::BodyTables>>,
    /// Unit event handler lists (unit +0x90, `bodies.md` §2.13), first =
    /// head.
    pub handlers: BTreeMap<UnitId, Vec<crate::skills::use_::bodies::Handler>>,
    /// The unit event iteration `0x005C0C30` on [`Self::handlers`]
    /// ([`combat::UnitEventFn`]). `None` (the default): every unit event
    /// goes to [`Pending::unit_event`];
    /// `ActionHooks::enable_unit_events` (a host with
    /// [`crate::wiring::interaction::UseRest`]) turns the registry on.
    pub unit_events: Option<combat::UnitEventFn<X>>,
    /// The client vitals sync's caches ([`vitals_sync`], `vitals.md` §5).
    /// `None` (the default): the sync is off;
    /// [`ActionHooks::enable_vitals_sync`] turns it on.
    pub sync: Option<vitals_sync::SyncState>,
    /// Client +0x508 of the players' clients ([`death`], `vitals.md`
    /// §4.6–§4.7).
    pub death: death::DeathState,
    /// The session state of the clients and players
    /// (`sim/intents-events.md` §8; [`switch`]): player names, hot keys,
    /// skill hands, portal flags.
    pub session: switch::SessionState,
    /// The players' server skill lists (unit +0xA8,
    /// [`crate::skills::list`]): created by the player init
    /// (`client/msg-skills.md` §2 rule 8) and filled by the character
    /// load (`formats/d2s.md` §7.2); the join's S→C 0x94 reads them.
    /// TODO(skills/levels.md): the combat and skill seams
    /// ([`Pending::skill_list`]) do not read them yet.
    pub skill_lists: BTreeMap<UnitId, crate::skills::list::SkillList>,
    /// The inactive-unit store (game +0xD8, `units.md` §3.4;
    /// [`inactive`]). `None` (the default): tick step 9 compresses
    /// nothing and the restore is the host's, as before.
    pub inactive: Option<crate::units::inactive::InactiveStore>,
    /// Seams with no provider yet.
    pub x: X,
    /// Scratch seed handed out for a unit without a record (an error is
    /// logged with it).
    pub orphan_seed: Seed,
    pub errors: Vec<WiringError>,
}

impl<X> ActionHooks<X> {
    pub fn new(tables: Arc<ActionTables>, drlg: DrlgWorld, game_seed: Seed, x: X) -> Self {
        Self {
            tables,
            drlg,
            missiles: Some(MissileStore::new()),
            ai: Some(AiStore::new()),
            ai_info: GameInfo::default(),
            combat_lists: BTreeMap::new(),
            hit_class: 0,
            game_seed,
            uniques: crate::items::UniqueBits::default(),
            items: crate::wiring::economy::ItemStore::new(),
            waypoints: BTreeMap::new(),
            objects: None,
            objects_out: false,
            object_drops: None,
            pet_follows: None,
            pet_deaths: None,
            owner_deaths: None,
            hireling_calls: None,
            anim_data: None,
            vitals: None,
            mode_target: None,
            monster_request: 0,
            monster_world: None,
            monster_world_out: false,
            quest_host: None,
            quest_host_out: false,
            deferred_inits: None,
            alloc_rooms: Vec::new(),
            paths: None,
            bodies: None,
            handlers: BTreeMap::new(),
            unit_events: None,
            sync: None,
            death: death::DeathState::default(),
            session: switch::SessionState::default(),
            skill_lists: BTreeMap::new(),
            inactive: None,
            x,
            orphan_seed: Seed::init(),
            errors: Vec::new(),
        }
    }

    /// Turns the path provider on ([`crate::wiring::path`]): from now on
    /// the path seams are answered by `d2_sim::path` instead of
    /// [`Pending`]. Call before any unit is allocated (units allocated
    /// earlier have no path record).
    pub fn enable_paths(&mut self) -> Result<(), crate::path::PathError> {
        self.paths = Some(Box::new(crate::wiring::path::PathState::new()?));
        Ok(())
    }

    /// Sets the host's `GetTickCount` the object calls read
    /// (`objects.md` edge case 9: an input of `d2-sim`, never read by
    /// it). No object state: nothing.
    pub fn set_host_tick(&mut self, ms: u32) {
        if let Some(st) = self.objects.as_mut() {
            st.host_tick = ms;
        }
    }

    /// The missile store (outside a missile call).
    pub fn missile_store(&self) -> &MissileStore {
        self.missiles.as_ref().expect("not lent out")
    }

    /// The AI store (outside a think).
    pub fn ai_store(&mut self) -> &mut AiStore {
        self.ai.as_mut().expect("not lent out")
    }
}

/// The unit side of a seam call: unit records, stat lists, unit tables
/// and the shared state. Missile and AI seams are implemented on it
/// ([`missiles`], [`ai`]); the game comes with each call.
pub struct View<'a, X> {
    pub units: &'a mut Units,
    pub stats: &'a mut StatLists,
    pub data: &'a UnitData,
    pub h: &'a mut ActionHooks<X>,
}

impl<'a, X> View<'a, X> {
    /// A view over a [`Sim`]'s parts (the game stays with the caller).
    pub fn of(
        units: &'a mut Units,
        stats: &'a mut StatLists,
        data: &'a UnitData,
        h: &'a mut ActionHooks<X>,
    ) -> Self {
        Self {
            units,
            stats,
            data,
            h,
        }
    }

    /// The [`Sim`] of a unit operation on `game`.
    pub fn sim<'b>(&'b mut self, game: &'b mut Game) -> Sim<'b> {
        Sim {
            game,
            units: self.units,
            stats: self.stats,
            data: self.data,
        }
    }
}
