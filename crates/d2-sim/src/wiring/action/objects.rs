// Spec: specs/world/objects.md §2, §3, §7, §14 (seam `ObjectWorld`); specs/audio/triggers-2.md §14; specs/world/waypoints.md §5.2 (the C→S 0x13 object case)
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
use crate::path::record::ObjectShape;
use crate::wiring::economy::drop_helpers;
use d2_data::tables::Objects;

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
    /// The init runs before `SUNIT_Add` (`units.md` §3.1 r7.1–r7.3): the
    /// unit is in no list and has no path record yet. Its room is the
    /// allocation's ([`View::init_room`], r7.2); (x, y) the allocation's
    /// when it went through [`View::create_object`], else (0, 0).
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
        let room = self.init_room(game, unit);
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

    /// `0x00623830` for `object` outside an object call (the quest
    /// code's collision free, `quests-act2.md` §8, `quests-act3.md`):
    /// [`apply_object_footprint`] with the object state's tables;
    /// `false`: no object state or `object` is not an object.
    pub fn free_object_footprint(&mut self, game: &Game, object: UnitId) -> bool {
        let Some(st) = self.h.objects.as_ref() else {
            return false;
        };
        let Some(r) = self.units.get(object).filter(|r| r.ty == UnitType::Object) else {
            return false;
        };
        let Ok(o) = st.tables.object(r.class as u16).cloned() else {
            return false;
        };
        let room = game.lists.unit(object).and_then(|e| e.room());
        let (x, y) = self.h.path_position(object);
        apply_object_footprint(&mut self.h.drlg, &o, room, x, y, false);
        true
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
    /// Result after the operate (`objects.md` §7.3 rule 5): the entry's
    /// result 0 → 3, else 0; after the walk 0.
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
        let (result, d) = self.operate_object(game, Some(player), guid)?;
        // Objects §7.3 rule 5: the entry's result 0 (object gone) → 3.
        if result == 0 {
            return Some(ObjectCase::Code(3));
        }
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
    /// ([`Pending::send`]), 0x60 likewise,
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
                UpdateMessage::Portal(b) => self.h.x.send(receiver, &b),
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

/// The §3 inputs of an objects row (`sim/path-placement.md` §3: size
/// `SizeX` × `SizeY`, the footprint mask from `IsDoor`, `BlocksVis`,
/// `BlockMissile`, `SubClass`; `HasCollision0..7`).
pub fn object_shape(o: &Objects) -> ObjectShape {
    ObjectShape {
        size_x: o.sizex,
        size_y: o.sizey,
        is_door: o.isdoor != 0,
        blocks_vis: o.blocksvis != 0,
        block_missile: o.blockmissile != 0,
        sub_class: u32::from(o.subclass),
        has_collision: [
            o.hascollision0,
            o.hascollision1,
            o.hascollision2,
            o.hascollision3,
            o.hascollision4,
            o.hascollision5,
            o.hascollision6,
            o.hascollision7,
        ]
        .map(|b| b != 0),
    }
}

/// The object footprint stamp `0x00620A70` (`set`) and free `0x00623830`
/// (`objects.md` §5.5, §8.2, §10; `objects-2.md` §16.13, §18.6): the
/// `SizeX` × `SizeY` box with the class's footprint mask (§3,
/// `0x006209D0`) set (`0x0064DE30`) or cleared (`0x0064DC00`) at (x, y),
/// cells looked up from `room` (`sim/path-placement.md` §4 rules 4–5,
/// §5.1); no room → nothing.
// PROVISIONAL (objects.md §8.2, §10; path-placement.md §5.2; REC-none):
// the free `0x00623830` is read as the box clear `0x0064DC00` with the
// class's footprint mask at the object's room and position, with no
// `HasCollision` test (its callers test `HasCollision` themselves;
// `sim/units.md` §3.1 r7.3 names the `0x0064DC00` call). Settled by a
// collision-grid capture around a door opening and a chest opening.
pub fn apply_object_footprint(
    drlg: &mut super::DrlgWorld,
    o: &Objects,
    room: Option<RoomId>,
    x: i32,
    y: i32,
    set: bool,
) {
    let s = object_shape(o);
    crate::path::collision::box_apply(drlg, room, x, y, (s.size_x, s.size_y), s.foot_mask(), set);
}

impl<X: Pending> ObjectView<'_, X> {
    /// The objects row of an object unit's class (unit record).
    fn object_row(&self, unit: UnitId) -> Option<Objects> {
        let r = self.v.units.get(unit)?;
        if r.ty != UnitType::Object {
            return None;
        }
        self.tables.object(r.class as u16).ok().cloned()
    }

    /// Runs a drop helper (`objects-2.md` §20,
    /// [`crate::wiring::economy::drop_helpers`]) with the drop state of
    /// [`super::ActionHooks::object_drops`] lent out and the object
    /// tables' `levels`; `None`: the game has no drop state. Without the
    /// path provider's field the floor search finds nothing
    /// ([`crate::wiring::economy::NoSpot`]): the picks still draw, no
    /// item is created.
    fn with_drops<R>(
        &mut self,
        f: impl FnOnce(
            &mut super::ActionHooks<X>,
            &mut crate::units::hooks::Sim<'_>,
            &mut crate::wiring::economy::DeathDrops,
            &[d2_data::tables::Levels],
            &mut crate::wiring::economy::NoSpot,
        ) -> R,
    ) -> Option<R> {
        let mut d = self.v.h.object_drops.take()?;
        let t = self.tables.clone();
        let out = {
            let mut sim = crate::units::hooks::Sim {
                game: &mut *self.game,
                units: &mut *self.v.units,
                stats: &mut *self.v.stats,
                data: self.v.data,
            };
            f(
                &mut *self.v.h,
                &mut sim,
                &mut d,
                &t.levels,
                &mut crate::wiring::economy::NoSpot,
            )
        };
        self.v.h.object_drops = Some(d);
        Some(out)
    }

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
    fn room_level(&self, room: RoomId) -> Option<u32> {
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
    /// `0x00620A70(O, room, x, y)` on the act DRLG's collision grids.
    fn stamp_footprint(&mut self, unit: UnitId, room: Option<RoomId>, x: i32, y: i32) {
        let Some(o) = self.object_row(unit) else {
            return;
        };
        apply_object_footprint(&mut self.v.h.drlg, &o, room, x, y, true);
    }
    /// `0x00623830` on the act DRLG's collision grids, at O's room and
    /// position.
    fn free_footprint(&mut self, unit: UnitId) {
        let Some(o) = self.object_row(unit) else {
            return;
        };
        let room = self.room(unit);
        let (x, y) = self.position(unit);
        apply_object_footprint(&mut self.v.h.drlg, &o, room, x, y, false);
    }
    /// `0x00553380(unit, id, to)` ([`crate::units::sound::queue_sound`],
    /// `audio/triggers-2.md` §14 rule 1). `now`: `0x00571740` at once
    /// (the chest's key sound, `objects.md` §8.1 rule 2, §14 rule 2) to
    /// the unit's own client when the unit is a player.
    fn sound(&mut self, unit: UnitId, id: u8, to: Option<UnitId>, now: bool) {
        if let Err(e) = crate::units::sound::queue_sound(self.game, unit, u16::from(id), to) {
            self.v.unit_error(crate::game::GameError::from(e).into());
        }
        let player = self
            .game
            .lists
            .unit(unit)
            .is_some_and(|e| e.ty == UnitType::Player);
        if now && player {
            // PROVISIONAL (audio/triggers-2.md §14 r2; REC-93; settled by a recording of a chest
            // unlocked with a key, count of 0x2C event 11): the slot and flag 0x400
            // stay set after the at-once send, so the player's unit update
            // of the same tick sends the event a second time.
            if let Some(m) = crate::units::sound::sound_message(self.game, unit, unit) {
                self.v.h.x.send(unit, &m);
            }
        }
    }
    fn key_test(&mut self, player: UnitId) -> bool {
        self.v.h.x.object_key_test(player)
    }
    fn in_interact_range(&self, operator: UnitId, object: UnitId) -> bool {
        self.v.h.x.object_in_range(self.game, operator, object)
    }
    /// `0x00554100`: the interact info on the player's unit record.
    fn interact_active(&self, player: UnitId) -> bool {
        self.v.units.get(player).is_some_and(|r| r.interact.active)
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
    /// Step 8 of an [`Self::allocate_object`] unit, after its init.
    fn add_object(&mut self, obj: UnitId, room: RoomId, x: i32, y: i32) {
        let req = crate::units::lifecycle::AllocRequest {
            ty: UnitType::Object,
            class: 0,
            room: Some(room),
            add: true,
            fixed_guid: None,
            mode: 0,
            allied: false,
        };
        self.v.add_allocated(self.game, obj, &req, x, y);
    }
    fn staff_tomb_level(&self) -> u32 {
        self.v.h.x.object_staff_tomb()
    }
    /// Unit +0x10 only (`objects-2.md` §18.1, §18.6).
    fn store_mode(&mut self, unit: UnitId, mode: u8) {
        if let Some(r) = self.record(unit) {
            r.mode = u32::from(mode);
        }
    }
    /// `0x00463740` on the act DRLG (`path-placement.md` §4 rule 1).
    fn room_at(&self, room: RoomId, x: i32, y: i32) -> Option<RoomId> {
        self.v.h.drlg.find_room(self.game, room, x, y)
    }
    /// `0x00559300` (`objects-2.md` §20.3) on the drop state.
    fn gold_drop(&mut self, room: RoomId, x: i32, y: i32) {
        self.with_drops(|h, sim, d, levels, spots| {
            drop_helpers::gold_drop(h, sim, d, levels, spots, Some(room), (x, y))
        });
    }
    /// `0x0064D800` with sizes 1, 1: the point query.
    fn point_free(&self, room: RoomId, x: i32, y: i32, mask: u32) -> bool {
        crate::path::collision::point_value(&self.v.h.drlg, Some(room), x, y, mask as u16) == 0
    }
}

/// The part-2 seams (`objects-2.md` §16–§18) on the unit lists, records,
/// the act DRLG and the path provider: interact (the unit record's
/// interact info), messages ([`Pending::send`]), adjacency, free
/// point and placement (`sim/path-placement.md` §7, §10), the 0x07 room
/// reveal, flags 2, the town test and the player lookup, the item drops
/// of §20 ([`drop_helpers`]). Trap damage, the gem test, the tome recount, the warp
/// tile, the day period keep their defaults.
impl<X: Pending> objects::MechWorld for ObjectView<'_, X> {
    /// `0x00554D00` on the unit record's interact info.
    fn interact_unit(&self, player: UnitId) -> Option<UnitId> {
        // Only the obelisk reads it, comparing with an object (§16.3).
        let (ty, g) = self.v.units.get(player)?.interact.get()?;
        (ty == UnitType::Object as u8)
            .then(|| self.game.lists.find_unit(UnitType::Object, g))
            .flatten()
    }
    fn set_interact(&mut self, player: UnitId, unit_type: u8, guid: u32) {
        if let Some(r) = self.v.units.get_mut(player) {
            r.interact.set(unit_type, guid);
        }
    }
    fn clear_interact(&mut self, player: UnitId) {
        if let Some(r) = self.v.units.get_mut(player) {
            r.interact.reset();
        }
    }
    fn send(&mut self, player: UnitId, msg: &[u8]) {
        self.v.h.x.send(player, msg);
    }
    fn adjacent_rooms(&self, room: RoomId) -> Vec<RoomId> {
        use crate::path::collision::CollisionRooms;
        let d = &self.v.h.drlg;
        (0..d.adjacent_count(room))
            .filter_map(|i| d.adjacent(room, i))
            .collect()
    }
    fn free_point(
        &self,
        room: RoomId,
        x: i32,
        y: i32,
        size: i32,
        mask: u32,
    ) -> Option<(RoomId, i32, i32)> {
        let mut p = crate::path::coords::Point::new(x, y);
        let rooms = crate::wiring::path::place::Rooms(&self.v.h.drlg);
        match crate::path::search::free_point(&rooms, Some(room), &mut p, size, mask, false) {
            Ok(Some(r)) => Some((r, p.x, p.y)),
            _ => None,
        }
    }
    fn place_unit(&mut self, unit: UnitId, room: RoomId, x: i32, y: i32) -> bool {
        if self.v.h.paths.is_none() {
            return false;
        }
        let c = crate::wiring::path::PathCtx::of(&mut self.v, self.game);
        crate::wiring::path::place::place_unit(c, unit, Some(room), x, y, false, false)
    }
    fn send_room_reveal(&mut self, player: UnitId, room: RoomId) {
        let Some((d, r)) = self.v.h.drlg.drlg_room(self.game, room) else {
            return;
        };
        let dr = d.room(r);
        let level = d.level(dr.level).id;
        let msg =
            crate::wiring::path::place::map_reveal(dr.rect.x as u16, dr.rect.y as u16, level as u8);
        self.v.h.x.send(player, &msg);
    }
    fn set_flags2(&mut self, unit: UnitId, bits: u32) {
        if let Some(r) = self.record(unit) {
            r.flags2 |= bits;
        }
    }
    fn in_town(&self, room: RoomId) -> bool {
        self.v.h.drlg.in_town(self.game, room)
    }
    fn find_player(&self, guid: u32) -> Option<UnitId> {
        self.game.lists.find_unit(UnitType::Player, guid)
    }
    /// `0x005594C0` / `0x00559630` with (room, &pos, −1, 0, 0) at the
    /// object (§16.5, §20.1, §20.2).
    fn stand_drop(&mut self, object: UnitId, weapon: bool) {
        let room = self.room(object);
        let pos = self.position(object);
        self.with_drops(|h, sim, d, levels, spots| {
            drop_helpers::stand_drop(h, sim, d, levels, spots, room, pos, weapon, -1, false, None)
        });
    }
    /// `0x00559A30(game, O, quality, &level, 0, −1, 0)` with O's drop
    /// code (§16.6, §20.4).
    fn drop_code_quality(&mut self, object: UnitId, code: u32, quality: u8) {
        self.with_drops(|h, sim, d, levels, spots| {
            drop_helpers::source_drop(h, sim, d, levels, spots, object, code, quality, -1, false)
        });
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
/// ([`crate::wiring::economy::object_chest_drop`]), the code drop
/// `0x00585970` and drop item code `0x00559A30` on the same drop state
/// ([`drop_helpers`]), unit type (unit
/// records), item quality (the game's item store), the room's units (the
/// room unit list). The rest keep their defaults (no spec body or no
/// provider: the key test, trap monsters and monster spawns
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
    /// `0x00585970(game, object, code, 0)` ([`drop_helpers::code_drop`],
    /// PROVISIONAL there).
    fn code_drop(&mut self, object: UnitId, code: u32) -> Option<UnitId> {
        self.with_drops(|h, sim, d, levels, spots| {
            drop_helpers::code_drop(h, sim, d, levels, spots, object, code, 0)
        })
        .flatten()
    }
    /// `0x00559A30` with the object's drop code (§8.1 rule 7, §20.4).
    // PROVISIONAL (objects.md §8.1 rule 7; REC-none): quality 2 (normal):
    // `items/quality.md` OQ2 gives 2 or 7 for every `0x00559A30` site
    // but the Cow King's, without naming the chest's; settled by a
    // capture of a chest with a drop code.
    fn drop_item_code(&mut self, object: UnitId, code: u32) {
        self.with_drops(|h, sim, d, levels, spots| {
            drop_helpers::source_drop(h, sim, d, levels, spots, object, code, 2, -1, false)
        });
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
    /// `0x0064D800(room, x, y, SizeX, SizeY, mask)` at O's room and
    /// position (§10; `object-population.md` §6: the box query, sizes
    /// ≤ 1 the point read): some cell has bits in `mask` (a cell without
    /// a room reads 0x27, `path-placement.md` §4 rule 2).
    fn footprint_collides(&self, object: UnitId, mask: u16) -> bool {
        let Some(o) = self.object_row(object) else {
            return false;
        };
        let Some(room) = self.room(object) else {
            return true;
        };
        let (x, y) = self.position(object);
        use objects::populate::PopulateWorld;
        self.box_query(room, x, y, o.sizex, o.sizey, u32::from(mask)) != 0
    }
    fn party_id(&self, unit: UnitId) -> u16 {
        self.v.h.x.object_party_id(unit)
    }
    fn portal_partner(&mut self, object: UnitId) -> Option<UnitId> {
        self.v.h.x.object_portal_partner(self.game, object)
    }
    fn has_quest_record(&self, player: UnitId) -> bool {
        self.v.h.x.object_quest_record(player)
    }
    fn expansion(&self) -> bool {
        self.v.data.expansion
    }
    fn player_quest_bit(&self, player: UnitId, quest: u32, bit: u8) -> bool {
        self.v.h.x.object_quest_bit(player, quest, bit)
    }
    fn player_portal_guid(&self, player: UnitId) -> u32 {
        self.v.h.x.object_portal_guid(player)
    }
    fn level_spawn_point(&mut self, level: u32) -> Option<(RoomId, i32, i32)> {
        self.v.h.x.object_level_spawn(self.game, level)
    }
    fn quest_level_change(&mut self, player: UnitId, from: u32, to: u32) {
        self.v.h.x.object_quest_level_change(player, from, to);
    }
    /// `0x005809D0` with the path provider ([`crate::wiring::path`]).
    fn player_mode_xy(&mut self, player: UnitId, mode: u8, x: i32, y: i32) {
        if self.v.h.paths.is_some() {
            let mut c = crate::wiring::path::PathCtx::of(&mut self.v, self.game);
            c.walk_to(player, u32::from(mode), x, y);
        }
    }
    fn remove_portal(&mut self, object: UnitId) {
        self.v.h.x.object_remove_portal(self.game, object);
    }
    fn portal_act5_hook(&mut self, partner: UnitId) {
        self.v.h.x.object_portal_act5(partner);
    }
    fn just_portaled(&mut self, player: UnitId, expire: i32) {
        self.v.h.x.object_just_portaled(self.game, player, expire);
    }
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
