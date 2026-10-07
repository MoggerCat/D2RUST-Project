// Spec: specs/world/objects.md §2, §3, §7, §14 (seam `ObjectWorld`); specs/world/waypoints.md §5.2 (the C→S 0x13 object case)
//! Objects ↔ units, timers, the unit lists and the DRLG: [`ObjectView`]
//! implements [`ObjectWorld`] (and the extension traits, on their
//! defaults). The object control and tables live in [`ObjectState`]
//! ([`super::ActionHooks::objects`]), built at game creation
//! ([`super::ActionSim::create_objects`]) and lent to each object call
//! ([`with_objects`]).
//!
//! Real providers: the frame, GUIDs and the object lookup (unit lists),
//! the operator kind, mode, animation, seed and flags (unit records), the
//! update queue (lists), room and level (DRLG), position (path), timers
//! (game timer queue), the host tick ([`ObjectState::host_tick`], set by
//! the host: never read by `d2-sim`). Footprints, sounds, the key test,
//! the interact range, the cursor and player-data tests, allocation from
//! inside an object call and the staff-tomb level go to [`Pending`].
//!
//! Routes into the object module: unit allocation of type 2
//! ([`super::units`] `init_kind` → [`objects::create`]), the object
//! timer events (`object_event` → [`objects::object_event`]), the
//! waypoint mode changes ([`super::waypoints`] → [`objects::set_object_mode`]),
//! the monster door operate ([`super::ai`] → [`objects::operate_in_range`]),
//! the per-client update pass ([`super::dispatch`] →
//! [`objects::update_messages`]) and the C→S 0x13 object case
//! ([`super::ActionSim::operate_object_message`]). What the module hands
//! back (quest, waypoint, `todo` routes) goes to [`Pending::object_route`],
//! or, for a quest route of a host holding the quest control
//! ([`ObjectState::route_quests`]), to that host's queue
//! ([`QuestObjectCall`]).

use std::sync::Arc;

use crate::game::Game;
use crate::rng::Seed;
use crate::units::record::flags;
use crate::units::{RoomId, UnitId, UnitType};
use crate::world::objects::{
    self, ChestWorld, Created, Dispatch, EventRun, MiscWorld, ObjectControl, ObjectError,
    ObjectTables, ObjectWorld, Operate, Operator, Preset, ShrineWorld, UpdateMessage,
};

use super::{Pending, View, WiringError};

/// The game's object state: the object control (game +0x10F0,
/// `objects.md` §2) with the per-object data, the tables, and the host
/// inputs of an object call.
#[derive(Debug, Clone)]
pub struct ObjectState {
    pub control: ObjectControl,
    /// `dwObjSeed`, game +0x80: the result `lo'` of `0x00546C60`
    /// (`objects.md` §2 rule 2, `rng.md` §5.2; S→C 0x03 u32@8,
    /// `client/model.md` §11 rule 1).
    pub obj_seed: u32,
    pub tables: Arc<ObjectTables>,
    /// The host's `GetTickCount` (edge case 9: an input the host sets,
    /// [`super::ActionHooks::set_host_tick`]).
    pub host_tick: u32,
    /// The allocation's (x, y) while [`View::create_object`] allocates
    /// (the per-kind init has no position argument).
    alloc_at: Option<(i32, i32)>,
    /// The quest routes for a host that holds the quest control
    /// ([`ObjectState::route_quests`]); `None` (the default): every
    /// route goes to [`Pending::object_route`].
    quest_calls: Option<Vec<QuestObjectCall>>,
}

/// A quest route of the object module (a quest init, operate or object
/// event 7), queued for the host that holds the quest control
/// (`d2_sim::wiring::economy::quest_objects`), with what the quest
/// function reads beyond the route: the object's class and the init's
/// room and position (`quests-act1-rest.md` §3: the marker init's
/// arguments).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestObjectCall {
    pub route: ObjectRoute,
    pub class: u16,
    pub room: Option<RoomId>,
    pub x: i32,
    pub y: i32,
}

/// The quest control of a host, lent to the hooks
/// ([`super::ActionHooks::quest_host`]) so that the quest routes of the
/// object module run where 1.14d runs them: a quest init inside the
/// object's allocation `0x00555230` (after the unit's seed step, before
/// the `PreOperate` draw and the add-to-world; `quests-act1-rest.md` §9
/// item 7, `objects.md` §3 rule 6), an operate inside the dispatch, object
/// event 7 inside the timer event. The provider is
/// `crate::wiring::economy::QuestLoan`.
pub trait QuestObjectHost<X> {
    /// Runs one route now on the game's parts; `Some`: a route no quest
    /// spec states, for [`Pending::object_route`].
    fn run(
        &mut self,
        game: &mut Game,
        v: &mut View<'_, X>,
        call: QuestObjectCall,
    ) -> Option<ObjectRoute>;
    /// For the host taking its parts back.
    fn into_any(self: Box<Self>) -> Box<dyn std::any::Any>;
}

/// What the object module handed back to its caller.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectRoute {
    /// A created object whose init function is a quest, waypoint or
    /// `todo` function (`object-functions.tsv` owner).
    Init { object: UnitId, created: Created },
    /// An operate dispatched to a quest, waypoint or `todo` function by a
    /// caller that does not run it here (the monster door operate; the
    /// 0x13 case hands the waypoint route to the caller instead).
    Operate(Dispatch),
    /// A quest or not-covered object timer event.
    Event { object: UnitId, run: EventRun },
    /// A preset class whose handler is not covered (§6).
    Preset {
        room: RoomId,
        class: u32,
        x: i32,
        y: i32,
    },
}

/// What the C→S 0x13 object case decides before the operate
/// (`waypoints.md` §5.2, `0x00548B00`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectReach {
    /// In range and unobstructed: the player is stopped (by the host's
    /// [`Pending::object_approach`]) and the operate runs now.
    Operate,
    /// Distance > 50: result 1.
    TooFar,
    /// Walk to the object; the operate on arrival is the host's.
    Walk,
}

/// The result of the C→S 0x13 object case.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectCase {
    /// The handler's result code.
    Code(u32),
    /// Operate 23: the caller runs `waypoints.md` §5.2 (the waypoint
    /// tables live with the host); then result 0.
    Waypoint(Operate),
}

/// The object code's view of a game.
pub struct ObjectView<'a, X> {
    pub game: &'a mut Game,
    pub v: View<'a, X>,
    /// [`ObjectState::host_tick`] of the call.
    pub host_tick: u32,
    /// The object tables of the call (the `levels` rows of the chest
    /// drop).
    pub tables: Arc<ObjectTables>,
}

/// A shorter-lived [`View`] over the same parts.
pub fn reborrow<'b, X>(v: &'b mut View<'_, X>) -> View<'b, X> {
    View {
        units: &mut *v.units,
        stats: &mut *v.stats,
        data: v.data,
        h: &mut *v.h,
    }
}

/// Runs `f` with the object control and tables lent out of
/// [`super::ActionHooks::objects`] and an [`ObjectView`] over `game` and
/// `v`. `None`: the game has no object state (not created), or it is
/// lent (a re-entrant call; an error is logged).
pub fn with_objects<X: Pending, R>(
    game: &mut Game,
    v: &mut View<'_, X>,
    f: impl FnOnce(&mut ObjectControl, &ObjectTables, &mut ObjectView<'_, X>) -> R,
) -> Option<R> {
    if v.h.objects_out {
        v.h.errors.push(WiringError::Reentrant("objects"));
        return None;
    }
    let mut st = v.h.objects.take()?;
    v.h.objects_out = true;
    let t = st.tables.clone();
    let r = {
        let mut ov = ObjectView {
            game,
            v: reborrow(v),
            host_tick: st.host_tick,
            tables: t.clone(),
        };
        f(&mut st.control, &t, &mut ov)
    };
    v.h.objects = Some(st);
    v.h.objects_out = false;
    Some(r)
}

/// Logs an object module error.
fn log<T, X>(v: &mut View<'_, X>, r: Result<T, ObjectError>) -> Option<T> {
    match r {
        Ok(t) => Some(t),
        Err(e) => {
            v.h.errors.push(WiringError::Object(e));
            None
        }
    }
}

impl ObjectState {
    /// `0x00546C60` (§2) on `game_seed` (one step).
    pub fn new(game_seed: &mut Seed, tables: Arc<ObjectTables>) -> Self {
        let (control, obj_seed) = ObjectControl::new(game_seed, &tables);
        Self {
            control,
            obj_seed,
            tables,
            host_tick: 0,
            alloc_at: None,
            quest_calls: None,
        }
    }

    /// Queues the quest routes for the host instead of handing them to
    /// [`Pending::object_route`] (a host with a quest control turns this
    /// on; idempotent).
    pub fn route_quests(&mut self) {
        self.quest_calls.get_or_insert_with(Vec::new);
    }

    /// The queued quest routes, in queue order (empty when off).
    pub fn take_quest_calls(&mut self) -> Vec<QuestObjectCall> {
        self.quest_calls
            .as_mut()
            .map(std::mem::take)
            .unwrap_or_default()
    }
}

impl<X: Pending> View<'_, X> {
    /// Hands a route back: a quest route to the host's queue when it is
    /// on ([`ObjectState::route_quests`]), anything else (and every route
    /// when it is off) to [`Pending::object_route`].
    pub fn object_route(
        &mut self,
        game: &mut Game,
        route: ObjectRoute,
        room: Option<RoomId>,
        (x, y): (i32, i32),
    ) {
        let object = match route {
            ObjectRoute::Init {
                object,
                created:
                    Created {
                        init: objects::Route::Quest,
                        ..
                    },
            } => Some(object),
            ObjectRoute::Operate(Dispatch::Quest(op)) => Some(op.object),
            ObjectRoute::Event {
                object,
                run: EventRun::Quest,
            } => Some(object),
            _ => None,
        };
        let class = object
            .and_then(|o| self.units.get(o))
            .map(|r| u16::try_from(r.class).unwrap_or(u16::MAX));
        let Some(class) = class else {
            return self.h.x.object_route(game, route);
        };
        let call = QuestObjectCall {
            route,
            class,
            room,
            x,
            y,
        };
        if self.h.quest_host.is_some() {
            return self.run_quest_host(game, call);
        }
        let out = self.h.quest_host_out;
        let queue = self.h.objects.as_mut().and_then(|s| {
            if out {
                // Raised inside a lent route: run right after it.
                s.route_quests();
            }
            s.quest_calls.as_mut()
        });
        match queue {
            Some(q) => q.push(call),
            None => self.h.x.object_route(game, route),
        }
    }

    /// Runs `call` on the lent quest host now, then the routes it raised
    /// (queued while it ran: a quest function that allocates a quest
    /// object, `quests-act1-rest.md` §3 step 3), in order.
    fn run_quest_host(&mut self, game: &mut Game, call: QuestObjectCall) {
        let Some(mut host) = self.h.quest_host.take() else {
            return;
        };
        self.h.quest_host_out = true;
        let mut calls = vec![call];
        while !calls.is_empty() {
            for c in calls {
                if let Some(r) = host.run(game, self, c) {
                    self.h.x.object_route(game, r);
                }
            }
            calls = self
                .h
                .objects
                .as_mut()
                .map(|s| s.take_quest_calls())
                .unwrap_or_default();
        }
        self.h.quest_host_out = false;
        self.h.quest_host = Some(host);
    }

    /// The per-kind init of an object allocation (`units.md` §3.1, §1
    /// table: the object data and `0x0054F5D0`, `objects.md` §3) on the
    /// object state; a game without one keeps the default (nothing).
    /// A quest, waypoint or `todo` init goes to [`Pending::object_route`].
    ///
    /// TODO(objects.md §3, units.md §3.1 step 8): d2rs adds the unit to
    /// the lists before the per-kind init (see `lifecycle::allocate`) and
    /// places its path after it: an init's footprint stamp sees no path
    /// record yet. The init's room is the list room; (x, y) the
    /// allocation's when it went through [`View::create_object`], else
    /// (0, 0).
    pub fn object_init(&mut self, game: &mut Game, unit: UnitId) {
        // An allocation from inside an object call: the caller holds the
        // control and runs §3 itself (`objects::allocate`).
        if self.h.objects_out {
            return;
        }
        let Some(r) = self.units.get(unit) else {
            return;
        };
        let (class, mode) = (r.class, r.mode);
        let guid = r.guid;
        let room = game.lists.unit(unit).and_then(|e| e.room());
        let (x, y) = self
            .h
            .objects
            .as_ref()
            .and_then(|s| s.alloc_at)
            .unwrap_or((0, 0));
        // Class and mode bounds are the dispatcher's fatal checks.
        let class = u16::try_from(class).unwrap_or(u16::MAX);
        let mode = u8::try_from(mode).unwrap_or(u8::MAX);
        let r = with_objects(game, self, |ctl, t, w| {
            objects::create_init(ctl, t, w, unit, class, guid, room, mode, x, y)
        });
        let Some(created) = r.and_then(|r| log(self, r)) else {
            return;
        };
        let handed = !matches!(created.init, objects::Route::Here | objects::Route::Null);
        let route = ObjectRoute::Init {
            object: unit,
            created,
        };
        // A quest init on a lent quest host runs at rule 6, before rules
        // 7–9 (`quests-act1-rest.md` §9 item 7); any other route is
        // handed back after them, as before.
        let now = handed && created.init == objects::Route::Quest && self.h.quest_host.is_some();
        if now {
            self.object_route(game, route, room, (x, y));
        }
        let r = with_objects(game, self, |ctl, t, w| {
            objects::create_rest(ctl, t, w, unit, mode)
        });
        if let Some(r) = r {
            log(self, r);
        }
        if handed && !now {
            self.object_route(game, route, room, (x, y));
        }
    }

    /// An object of `class` allocated in `room` at (x, y) in `mode`
    /// (`0x00555230(type 2, …)`), its init run ([`View::object_init`]);
    /// preset classes 574–582 go through `0x0054F490`
    /// ([`objects::create_preset`]). `None`: no object state (the caller
    /// keeps its [`Pending`] answer), or nothing allocated.
    pub fn create_object(
        &mut self,
        game: &mut Game,
        room: RoomId,
        class: u32,
        x: i32,
        y: i32,
        mode: u8,
    ) -> Option<UnitId> {
        let st = self.h.objects.as_mut()?;
        if class > u32::from(objects::CLASS_BOUND) {
            let level = self.h.drlg.level_id(game, room).unwrap_or(0);
            let r = with_objects(game, self, |ctl, t, w| {
                objects::create_preset(ctl, t, w, room, level, class, x, y, mode)
            })?;
            return match log(self, r)? {
                Preset::Object(u) => u,
                Preset::None => None,
                Preset::NotCovered(class) => {
                    self.h
                        .x
                        .object_route(game, ObjectRoute::Preset { room, class, x, y });
                    None
                }
            };
        }
        st.alloc_at = Some((x, y));
        let req = crate::units::lifecycle::AllocRequest {
            ty: UnitType::Object,
            class,
            room: Some(room),
            add: true,
            fixed_guid: None,
            mode: u32::from(mode),
            allied: false,
        };
        let u = self.allocate(game, &req, x, y);
        if let Some(st) = self.h.objects.as_mut() {
            st.alloc_at = None;
        }
        u
    }

    /// An object timer event (`units.md` §6.4) on the object state.
    pub fn object_event(&mut self, game: &mut Game, unit: UnitId, ev: u8) {
        let r = with_objects(game, self, |ctl, t, w| {
            objects::object_event(ctl, t, w, unit, ev)
        });
        let Some(run) = r.and_then(|r| log(self, r)) else {
            return;
        };
        if run != EventRun::Done {
            let room = game.lists.unit(unit).and_then(|e| e.room());
            let at = self.h.path_position(unit);
            self.object_route(game, ObjectRoute::Event { object: unit, run }, room, at);
        }
    }

    /// Object population `0x00552610` of the room `info` describes
    /// (`object-population.md`) on the object state. `None`: no object
    /// state (the caller keeps its [`Pending`] answer).
    pub fn populate_objects(
        &mut self,
        game: &mut Game,
        info: &objects::populate::RoomInfo,
    ) -> Option<objects::populate::Populated> {
        self.h.objects.as_ref()?;
        let r = with_objects(game, self, |ctl, t, w| {
            objects::populate::populate_room(ctl, t, w, info)
        })?;
        log(self, r)
    }

    /// The object mode change `0x00624690` of an object with object data
    /// ([`objects::set_object_mode`]); `false`: no object state or no
    /// data for `object` (the caller keeps its [`Pending`] answer).
    pub fn object_set_mode(&mut self, game: &mut Game, object: UnitId, mode: u8) -> bool {
        let known = self
            .h
            .objects
            .as_ref()
            .is_some_and(|s| s.control.data.contains_key(&object));
        if !known {
            return false;
        }
        let r = with_objects(game, self, |ctl, t, w| {
            objects::set_object_mode(ctl, t, w, object, mode)
        });
        if let Some(r) = r {
            log(self, r);
        }
        true
    }

    /// The operate entry `0x00584540` (§7.1) by `operator` on the object
    /// with `guid`: the entry's result and the dispatch. `None`: no
    /// object state.
    pub fn operate_object(
        &mut self,
        game: &mut Game,
        operator: Option<UnitId>,
        guid: u32,
    ) -> Option<(i32, Option<Dispatch>)> {
        let r = with_objects(game, self, |ctl, t, w| {
            objects::operate_in_range(ctl, t, w, operator, guid)
        })?;
        log(self, r)
    }

    /// The C→S 0x13 object case `0x00548B00` (`waypoints.md` §5.2) for
    /// `player` and the object with `guid`: object missing → 1; mode ≥ 8
    /// → 3; [`Pending::object_approach`] (distance > 50 → 1; walk); in
    /// range → the operate entry (§7.1). Operate 23 is handed back
    /// ([`ObjectCase::Waypoint`]); quest and `todo` routes go to
    /// [`Pending::object_route`]. `None`: no object state.
    ///
    /// TODO(waypoints.md §5.2): the result after the operate (and after
    /// the walk) is not stated; read as 0.
    pub fn object_message(
        &mut self,
        game: &mut Game,
        player: UnitId,
        guid: u32,
    ) -> Option<ObjectCase> {
        self.h.objects.as_ref()?;
        let Some(object) = game.lists.find_unit(UnitType::Object, guid) else {
            return Some(ObjectCase::Code(1));
        };
        if self.units.get(object).map_or(0, |r| r.mode) >= u32::from(objects::MODE_BOUND) {
            return Some(ObjectCase::Code(3));
        }
        match self.h.x.object_approach(game, player, object) {
            ObjectReach::TooFar => return Some(ObjectCase::Code(1)),
            ObjectReach::Walk => return Some(ObjectCase::Code(0)),
            ObjectReach::Operate => {}
        }
        let (_, d) = self.operate_object(game, Some(player), guid)?;
        Some(match d {
            Some(Dispatch::Waypoint(op)) => ObjectCase::Waypoint(op),
            Some(d @ (Dispatch::Quest(_) | Dispatch::NotCovered(_))) => {
                let room = game.lists.unit(object).and_then(|e| e.room());
                let at = self.h.path_position(object);
                self.object_route(game, ObjectRoute::Operate(d), room, at);
                ObjectCase::Code(0)
            }
            Some(Dispatch::Done(_)) | None => ObjectCase::Code(0),
        })
    }

    /// The object update pass `0x00581AD0` (§14) for one queued object and
    /// the client of `receiver`: S→C 0x0E / 0x4D sent to `receiver`
    /// ([`Pending::send`]), 0x60 to [`Pending::object_portal_message`],
    /// then rule 2 ([`MiscWorld::update_extras`]). `false`: not an object
    /// with object data (nothing ran).
    pub fn object_update(&mut self, game: &mut Game, receiver: UnitId, unit: UnitId) -> bool {
        let known = self
            .h
            .objects
            .as_ref()
            .is_some_and(|s| s.control.data.contains_key(&unit));
        if !known {
            return false;
        }
        let r = with_objects(game, self, |ctl, t, w| {
            let msgs = objects::update_messages(ctl, t, w, unit);
            w.update_extras(unit);
            msgs
        });
        let Some(msgs) = r.and_then(|r| log(self, r)) else {
            return true;
        };
        for m in msgs {
            match m {
                UpdateMessage::State(b) => self.h.x.send(receiver, &b),
                UpdateMessage::Shrine(b) => self.h.x.send(receiver, &b),
                UpdateMessage::Portal(o) => self.h.x.object_portal_message(receiver, o),
            }
        }
        true
    }

    /// objects.txt `MonsterOK` of a door (`ai.md`); `None`: no object
    /// state or no data.
    pub fn object_monster_ok(&self, door: UnitId) -> Option<bool> {
        let st = self.h.objects.as_ref()?;
        let class = st.control.data.get(&door)?.class;
        st.tables.object(class).ok().map(|o| o.monsterok != 0)
    }
}

impl<X: Pending> ObjectView<'_, X> {
    fn record(&mut self, u: UnitId) -> Option<&mut crate::units::record::UnitRecord> {
        let r = self.v.units.get_mut(u);
        if r.is_none() {
            self.v.h.errors.push(WiringError::Unit(
                crate::units::modes::UnitError::UnknownUnit(u),
            ));
        }
        r
    }
}

impl<X: Pending> ObjectWorld for ObjectView<'_, X> {
    fn frame(&self) -> i32 {
        self.game.frame
    }
    fn host_tick(&self) -> u32 {
        self.host_tick
    }
    fn guid(&self, unit: UnitId) -> u32 {
        self.game.lists.unit(unit).map_or(0, |e| e.guid)
    }
    /// `0x00552F60`.
    fn find_object(&self, guid: u32) -> Option<UnitId> {
        self.game.lists.find_unit(UnitType::Object, guid)
    }
    fn operator(&self, unit: UnitId) -> Operator {
        match self.v.units.get(unit) {
            Some(r) if r.ty == UnitType::Player => Operator::Player(r.class as u8),
            Some(r) if r.ty == UnitType::Monster => Operator::Monster,
            _ => Operator::Other,
        }
    }
    fn mode(&self, unit: UnitId) -> u8 {
        self.v.units.get(unit).map_or(0, |r| r.mode as u8)
    }
    /// Unit +0x10, flag 0x1, and `0x0064C040` (`unit-order.md` §6.2).
    fn write_mode(&mut self, unit: UnitId, mode: u8, queue: bool) {
        let Some(r) = self.record(unit) else {
            return;
        };
        r.mode = u32::from(mode);
        r.flags |= flags::CHANGED;
        if queue {
            self.queue_update(unit);
        }
    }
    fn set_anim(&mut self, unit: UnitId, frame_count: i32, frame: i32, speed: i16) {
        if let Some(r) = self.record(unit) {
            r.anim.frame_count = frame_count;
            r.anim.frame = frame;
            r.anim.speed = speed;
        }
    }
    fn unit_seed(&mut self, unit: UnitId) -> Option<&mut Seed> {
        self.v.units.get_mut(unit).map(|r| &mut r.seed)
    }
    fn flags(&self, unit: UnitId) -> u32 {
        self.v.units.get(unit).map_or(0, |r| r.flags)
    }
    fn set_flags(&mut self, unit: UnitId, f: u32) {
        if let Some(r) = self.record(unit) {
            r.flags = f;
        }
    }
    fn queue_update(&mut self, unit: UnitId) {
        if let Err(e) = self.game.lists.queue_update(unit) {
            self.v.unit_error(crate::game::GameError::from(e).into());
        }
    }
    fn room(&self, unit: UnitId) -> Option<RoomId> {
        self.game.lists.unit(unit).and_then(|e| e.room())
    }
    fn level(&self, unit: UnitId) -> Option<u32> {
        let room = self.room(unit)?;
        self.v.h.drlg.level_id(self.game, room)
    }
    fn position(&self, unit: UnitId) -> (i32, i32) {
        self.v.h.path_position(unit)
    }
    /// `0x005417D0` (`tick.md` §5.2).
    fn schedule(&mut self, unit: UnitId, ev: u8, frame: i32) {
        if let Err(e) = self
            .game
            .schedule_event(unit, u32::from(ev), frame, None, 0, 0)
        {
            self.v.unit_error(e.into());
        }
    }
    /// `0x00540F30` (`tick.md` §5.4).
    fn cancel_timers(&mut self, unit: UnitId) {
        self.game.timers.cancel_unit_timers(unit);
    }
    fn stamp_footprint(&mut self, unit: UnitId) {
        self.v.h.x.object_stamp_footprint(self.game, unit);
    }
    fn free_footprint(&mut self, unit: UnitId) {
        self.v.h.x.object_free_footprint(self.game, unit);
    }
    fn sound(&mut self, unit: UnitId, id: u8, to: Option<UnitId>, now: bool) {
        self.v.h.x.object_sound(unit, id, to, now);
    }
    fn key_test(&mut self, player: UnitId) -> bool {
        self.v.h.x.object_key_test(player)
    }
    fn in_interact_range(&self, operator: UnitId, object: UnitId) -> bool {
        self.v.h.x.object_in_range(self.game, operator, object)
    }
    /// `0x00554100`: the interact info ([`Pending::interact_guid`], the
    /// waypoint seam's).
    fn interact_active(&self, player: UnitId) -> bool {
        self.v.h.x.interact_guid(player).is_some()
    }
    fn player_busy(&self, player: UnitId) -> bool {
        self.v.h.x.object_player_busy(player)
    }
    fn cursor_item(&self, player: UnitId) -> bool {
        self.v.h.x.object_cursor_item(player)
    }
    /// An allocation from inside an object call (§6 presets, §8.3 fire
    /// objects): the unit only; [`View::object_init`] sees the control
    /// lent and leaves §3 to [`objects::allocate`].
    fn allocate_object(
        &mut self,
        room: RoomId,
        class: u16,
        x: i32,
        y: i32,
        mode: u8,
    ) -> Option<UnitId> {
        let req = crate::units::lifecycle::AllocRequest {
            ty: UnitType::Object,
            class: u32::from(class),
            room: Some(room),
            add: true,
            fixed_guid: None,
            mode: u32::from(mode),
            allied: false,
        };
        self.v.allocate(self.game, &req, x, y)
    }
    fn staff_tomb_level(&self) -> u32 {
        self.v.h.x.object_staff_tomb()
    }
}

/// The population seams on the act DRLG: the active room seed (+0x6C),
/// the level's populated-room count (`0x0061ABF0` → `0x00642BE0`), the
/// collision queries of `0x0064D800` (`object-population.md` §6) and the
/// unit record's class.
impl<X: Pending> objects::populate::PopulateWorld for ObjectView<'_, X> {
    fn room_seed(&mut self, room: RoomId) -> Option<&mut Seed> {
        let act = self.game.lists.room(room)?.act;
        let d = self
            .v
            .h
            .drlg
            .dungeon
            .acts
            .get_mut(usize::from(act))?
            .as_mut()?;
        let r = d.drlg_room_of(room)?;
        d.active_room_seed_mut(r)
    }
    fn populated_room_count(&mut self, act: u8, level: u32) -> i32 {
        let r = self.v.h.drlg.with_act(act, &mut self.game.lists, |d, svc| {
            d.populated_room_count(svc.data, svc.types, level)
        });
        match r {
            Some(Ok(n)) => n as i32,
            Some(Err(e)) => {
                self.v.h.errors.push(WiringError::Drlg(e));
                0
            }
            None => 0,
        }
    }
    fn box_query(&self, room: RoomId, x: i32, y: i32, sx: u32, sy: u32, mask: u32) -> u32 {
        let mask = mask as u16;
        let drlg = &self.v.h.drlg;
        u32::from(if sx <= 1 && sy <= 1 {
            crate::path::collision::point_value(drlg, Some(room), x, y, mask)
        } else {
            crate::path::collision::box_value(drlg, Some(room), x, y, (sx, sy), mask)
        })
    }
    fn set_unit_class(&mut self, unit: UnitId, class: u16) {
        if let Some(r) = self.record(unit) {
            r.class = u32::from(class);
        }
    }
}

/// The chest seams on the providers the action wiring holds: the chest
/// drop on [`super::ActionHooks::object_drops`]
/// ([`crate::wiring::economy::object_chest_drop`]), unit type (unit
/// records), item quality (the game's item store), the room's units (the
/// room unit list). The rest keep their defaults (no spec body or no
/// provider: the key test, code drop `0x00585970` and drop item code
/// `0x00559A30` (items specs), trap monsters and monster spawns
/// (monsters specs), the free-spot search with mask 0x3F11, the player's
/// skill start, the range test's metric, trap damage, the "inside the
/// room" bound).
impl<X: Pending> ChestWorld for ObjectView<'_, X> {
    /// `0x00585B90` with the operate record (`treasure.md` §4). Without
    /// the path provider's field the free-spot search finds nothing
    /// ([`crate::wiring::economy::NoSpot`]): the walk still draws, no item
    /// is created.
    fn chest_drop(&mut self, op: &Operate, q: u8) -> Option<UnitId> {
        let mut d = self.v.h.object_drops.take()?;
        let t = self.tables.clone();
        let out = {
            let mut sim = crate::units::hooks::Sim {
                game: &mut *self.game,
                units: &mut *self.v.units,
                stats: &mut *self.v.stats,
                data: self.v.data,
            };
            crate::wiring::economy::object_chest_drop(
                &mut *self.v.h,
                &mut sim,
                &mut d,
                &t.levels,
                &mut crate::wiring::economy::NoSpot,
                op.object,
                op.operator,
                q,
            )
        };
        self.v.h.object_drops = Some(d);
        out
    }
    fn unit_type(&self, unit: UnitId) -> Option<u8> {
        self.v.units.get(unit).map(|r| r.ty as u8)
    }
    fn item_quality(&self, item: UnitId) -> Option<u8> {
        self.v.h.items.get(item).map(|i| i.quality)
    }
    fn room_units(&self, room: RoomId) -> Vec<UnitId> {
        self.game.lists.room_units(room)
    }
    /// The lent monster world's region (`population.md` §2.2).
    fn monster_region_classes(&self, level: u32) -> Option<Vec<i32>> {
        self.v.h.monster_world.as_ref()?.region_classes(level)
    }
    fn monstats_count(&self) -> u32 {
        self.v
            .h
            .monster_world
            .as_ref()
            .map_or(0, |m| m.monstats_count())
    }
    fn unit_class(&self, unit: UnitId) -> Option<u32> {
        self.v.units.get(unit).map(|r| r.class)
    }
    /// `0x00620510` for an object (`SizeX`); other units: not read here.
    fn unit_size(&self, unit: UnitId) -> i32 {
        self.v
            .units
            .get(unit)
            .filter(|r| r.ty == UnitType::Object)
            .and_then(|r| self.tables.object(r.class as u16).ok())
            .map_or(0, |o| o.sizex as i32)
    }
    fn room_rect(&self, room: RoomId) -> Option<(i32, i32, i32, i32)> {
        self.v
            .h
            .drlg
            .subtiles(self.game, room)
            .map(|r| (r.x, r.y, r.w, r.h))
    }
}
/// The shrine seams on the unit's stat list (`sim/stats.md`: getter
/// `0x00625480`, set `0x00627260`, add `0x006272B0`, the maxima
/// `0x00625D10` / `0x00625D60` / `0x00625DB0`, level = stat 12). The rest
/// keep their defaults (hovers, the timed-state helper and its list
/// writer, skill refresh, to-hit, two-handed test, gems and item drops,
/// the units in range (metric not stated), missiles, the free spot,
/// portals, the unique monster).
///
/// TODO(objects.md §9, sim/stats.md): "set with its client update": the
/// update message is not stated beyond the stat-list hooks; the set runs
/// through the stat list's host ([`super::ActionHooks`]) only.
impl<X: Pending> ShrineWorld for ObjectView<'_, X> {
    fn stat(&self, unit: UnitId, id: u16) -> i32 {
        self.v.stat(unit, id)
    }
    fn set_stat(&mut self, unit: UnitId, id: u16, value: i32) {
        self.v.set_base(unit, id, value);
    }
    fn add_base_stat(&mut self, unit: UnitId, id: u16, delta: i32) {
        self.v.stats.unit_add(&mut *self.v.h, unit, id, delta, 0);
    }
    fn max_life(&self, unit: UnitId) -> i32 {
        self.v.stats.max_life(unit)
    }
    fn max_mana(&self, unit: UnitId) -> i32 {
        self.v.stats.max_mana(unit)
    }
    fn max_stamina(&self, unit: UnitId) -> i32 {
        self.v.stats.max_stamina(unit)
    }
    fn player_level(&self, player: UnitId) -> i32 {
        self.v.stat(player, STAT_LEVEL)
    }
}

/// Stat 12 `level` (`sim/stats.md`).
const STAT_LEVEL: u16 = 12;

/// The door, well and portal seams on the unit's stat lists: the vitals
/// and their maxima (getters `0x00625D10`, `0x00625D60`, `0x00625DB0`),
/// the vital set (`0x00627260`), the state list removal (`0x006256B0`,
/// free) and `0x00578C20` (`world/npc.md` §5 step 4: every curable state
/// with a list). The rest keep their defaults (portal creation, update
/// extras, the footprint test, the pet heal, players, hostility, portal
/// travel).
///
/// TODO(objects.md §11 rule 2): the client update of the vital set is
/// the stat list host's; no separate message is sent here.
impl<X: Pending> MiscWorld for ObjectView<'_, X> {
    fn vital_stat(&self, unit: UnitId, id: u16) -> u32 {
        let st = &*self.v.stats;
        (match id {
            7 => st.max_life(unit),
            9 => st.max_mana(unit),
            11 => st.max_stamina(unit),
            _ => st.unit_total(unit, id, 0),
        }) as u32
    }
    fn set_vital_stat(&mut self, unit: UnitId, id: u16, value: u32) {
        self.v.set_base(unit, id, value as i32);
    }
    fn remove_state_list(&mut self, unit: UnitId, state: u16) -> bool {
        if self.v.state_list(unit, state).is_none() {
            return false;
        }
        self.v
            .stats
            .free_state_list(&mut *self.v.h, unit, u32::from(state));
        true
    }
    fn cure_states(&mut self, unit: UnitId) -> bool {
        let count = self.v.stats.data().states.count();
        let mut changed = false;
        for s in 0..count {
            let Ok(s16) = u16::try_from(s) else {
                break;
            };
            let st = u32::from(s16);
            let curable = self
                .v
                .stats
                .data()
                .states
                .has_flag(st, crate::wiring::interaction::npc_world::STATE_CURABLE);
            if self.v.stats.has_state(unit, st) && curable && self.v.state_list(unit, s16).is_some()
            {
                self.v.stats.free_state_list(&mut *self.v.h, unit, st);
                changed = true;
            }
        }
        changed
    }
}
