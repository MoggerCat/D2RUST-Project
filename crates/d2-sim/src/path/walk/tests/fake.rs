//! Seam fakes: one room [0, W) × [0, H) with a collision grid that
//! follows `path-placement.md` §3–§6 for patterns 0–5 (the rules the
//! pathing.md test vectors assume), and a unit table.

use std::collections::BTreeMap;

use crate::game::Game;
use crate::path::walk::find::set_type;
use crate::path::walk::geom::centre;
use crate::path::walk::seams::{PathWorld, Point, StartTarget, UsedSkill, WalkPath, WalkUnits};
use crate::path::walk::tables::PathTables;
use crate::rng::Seed;
use crate::units::{ClientId, RoomId, UnitId, UnitType};

pub const WALL: u16 = 0x1;
pub const NO_ROOM: u16 = 0x27;
pub const ROOM: RoomId = RoomId(0);

pub struct FakeWorld {
    pub w: i32,
    pub h: i32,
    pub grid: Vec<u16>,
    pub town: bool,
    /// Rect of [`ROOM`]; the grid may be larger (cells of other rooms).
    pub room0: (i32, i32, i32, i32),
    pub paths: BTreeMap<UnitId, WalkPath>,
    pub clients: BTreeMap<RoomId, Vec<ClientId>>,
    /// Extra rooms: id → rect; cells outside the main room resolve here.
    pub rooms: BTreeMap<RoomId, (i32, i32, i32, i32)>,
    pub log: Vec<String>,
}

fn plus(x: i32, y: i32) -> [Point; 5] {
    [
        Point::new(x, y),
        Point::new(x - 1, y),
        Point::new(x + 1, y),
        Point::new(x, y - 1),
        Point::new(x, y + 1),
    ]
}

fn cells(pattern: u8, x: i32, y: i32) -> Option<Vec<Point>> {
    match pattern {
        0 => Some(vec![Point::new(x, y)]),
        1 | 3 | 5 => Some(plus(x, y).to_vec()),
        2 | 4 => {
            let mut v = Vec::new();
            for dy in -1..=1 {
                for dx in -1..=1 {
                    v.push(Point::new(x + dx, y + dy));
                }
            }
            Some(v)
        }
        _ => None,
    }
}

impl FakeWorld {
    pub fn new(w: i32, h: i32) -> FakeWorld {
        FakeWorld {
            w,
            h,
            grid: vec![0; (w * h) as usize],
            town: false,
            room0: (0, 0, w, h),
            paths: BTreeMap::new(),
            clients: BTreeMap::new(),
            rooms: BTreeMap::new(),
            log: Vec::new(),
        }
    }

    pub fn wall(&mut self, x: i32, y: i32) {
        let i = (y * self.w + x) as usize;
        self.grid[i] |= WALL;
    }

    fn inside(&self, p: Point) -> bool {
        p.x >= 0 && p.y >= 0 && p.x < self.w && p.y < self.h
    }

    pub fn value(&self, p: Point) -> u16 {
        if self.inside(p) {
            self.grid[(p.y * self.w + p.x) as usize]
        } else {
            NO_ROOM
        }
    }

    /// `0x0064D870`: OR of masked values (cell without a room: 0x27
    /// unmasked).
    fn query(&self, pattern: u8, at: Point, mask: u16) -> u16 {
        let Some(cs) = cells(pattern, at.x, at.y) else {
            return 0xFFFF;
        };
        cs.iter().fold(0, |acc, &c| {
            acc | if self.inside(c) {
                self.value(c) & mask
            } else {
                NO_ROOM
            }
        })
    }

    fn marker(pattern: u8) -> (u16, bool) {
        match pattern {
            1 => (0x1000, false),
            2 => (0x1000, true),
            3 => (0x2000, false),
            4 => (0x2000, true),
            _ => (0, false),
        }
    }

    pub fn stamp(&mut self, pattern: u8, at: Point, mask: u16, set: bool) {
        let Some(cs) = cells(pattern, at.x, at.y) else {
            return;
        };
        let (w, h) = (self.w, self.h);
        let inside = |c: Point| c.x >= 0 && c.y >= 0 && c.x < w && c.y < h;
        for c in cs.iter().copied().filter(|&c| inside(c)) {
            let i = (c.y * self.w + c.x) as usize;
            if set {
                self.grid[i] |= mask;
            } else {
                self.grid[i] &= !mask;
            }
        }
        if mask == 0 {
            return;
        }
        let (m, on_plus) = Self::marker(pattern);
        if m == 0 {
            return;
        }
        let mc: Vec<Point> = if on_plus {
            plus(at.x, at.y).to_vec()
        } else {
            vec![at]
        };
        for c in mc.into_iter().filter(|&c| inside(c)) {
            let i = (c.y * self.w + c.x) as usize;
            if set {
                self.grid[i] |= m;
            } else {
                self.grid[i] &= !m;
            }
        }
    }

    /// A player path as `path-placement.md` §2.4 allocates it, footprint
    /// stamped.
    pub fn add_player(&mut self, t: &PathTables, unit: UnitId, x: i32, y: i32) {
        let mut p = WalkPath::zeroed(unit);
        p.size = 2;
        p.pattern = t.pattern_of_size[2];
        p.precise_x = centre(x);
        p.precise_y = centre(y);
        p.velocity = 0x800;
        p.room = Some(ROOM);
        p.saved_count = 1;
        p.saved_steps[0] = Point::new(x, y);
        p.footprint_mask = 0x80;
        p.move_mask = 0x1C09;
        set_type(t, &mut p, UnitType::Player, 7).unwrap();
        p.max_distance = 73;
        p.idastar_score = 70;
        self.stamp(p.pattern, Point::new(x, y), p.footprint_mask, true);
        self.paths.insert(unit, p);
    }
}

impl PathWorld for FakeWorld {
    fn load_path(&self, unit: UnitId) -> Option<WalkPath> {
        self.paths.get(&unit).cloned()
    }
    fn store_path(&mut self, unit: UnitId, path: &WalkPath) {
        self.paths.insert(unit, path.clone());
    }
    fn cell_room(&self, room: Option<RoomId>, x: i32, y: i32) -> Option<RoomId> {
        room?;
        let within = |(rx, ry, rw, rh): (i32, i32, i32, i32)| {
            x >= rx && x < rx + rw && y >= ry && y < ry + rh
        };
        if within(self.room0) {
            return Some(ROOM);
        }
        self.rooms.iter().find(|(_, &r)| within(r)).map(|(r, _)| *r)
    }
    fn room_rect(&self, room: RoomId) -> (i32, i32, i32, i32) {
        if room == ROOM {
            self.room0
        } else {
            self.rooms.get(&room).copied().unwrap_or((0, 0, 0, 0))
        }
    }
    fn room_in_town(&self, _room: RoomId) -> bool {
        self.town
    }
    fn pattern_collides(
        &self,
        room: Option<RoomId>,
        x: i32,
        y: i32,
        pattern: u8,
        mask: u16,
    ) -> bool {
        if room.is_none() || pattern > 5 {
            return true;
        }
        self.query(pattern, Point::new(x, y), mask) != 0
    }
    fn remove_footprint(&mut self, unit: UnitId, _force: bool) -> bool {
        let Some(p) = self.paths.get(&unit).cloned() else {
            return false;
        };
        self.stamp(p.pattern, p.cell(), p.footprint_mask, false);
        true
    }
    fn add_footprint(&mut self, unit: UnitId) {
        if let Some(p) = self.paths.get(&unit).cloned() {
            self.stamp(p.pattern, p.cell(), p.footprint_mask, true);
        }
    }
    fn try_move(
        &mut self,
        _room: Option<RoomId>,
        old: Point,
        new: Point,
        pattern: u8,
        foot: u16,
        test: u16,
    ) -> u16 {
        self.stamp(pattern, old, foot, false);
        let r = self.query(pattern, new, test);
        if r != 0 {
            self.stamp(pattern, old, foot, true);
            return r;
        }
        self.stamp(pattern, new, foot, true);
        0
    }
    fn forced_move(
        &mut self,
        _room: Option<RoomId>,
        old: Point,
        new: Point,
        pattern: u8,
        foot: u16,
    ) {
        self.stamp(pattern, old, foot, false);
        self.stamp(pattern, new, foot, true);
    }
    fn missile_move(
        &mut self,
        _room: Option<RoomId>,
        _old: Point,
        _new: Point,
        _size: i32,
        _foot: u16,
        _test: u16,
    ) -> u16 {
        0
    }
    fn room_list_remove(&mut self, unit: UnitId, room: RoomId) {
        self.log.push(format!("leave {} {}", unit.0, room.0));
    }
    fn room_list_insert(&mut self, unit: UnitId, room: RoomId) {
        self.log.push(format!("insert {} {}", unit.0, room.0));
    }
    fn queue_for_update(&mut self, unit: UnitId) {
        self.log.push(format!("queue {}", unit.0));
    }
    fn room_clients(&self, room: RoomId) -> Vec<ClientId> {
        self.clients.get(&room).cloned().unwrap_or_default()
    }
}

#[derive(Clone)]
pub struct FakeUnit {
    pub ty: UnitType,
    pub class: u32,
    pub guid: u32,
    pub mode: u32,
    pub pos: Point,
    pub size: i32,
    pub stats: BTreeMap<u16, i32>,
    pub item_stats: BTreeMap<u16, i32>,
    pub states: Vec<u16>,
    pub state_stats: BTreeMap<(u16, u16), i32>,
    pub used_skill: Option<UsedSkill>,
    pub seed: Seed,
    pub cursor: bool,
    pub torso_speed: Option<i32>,
    pub run_bonus: i32,
}

impl FakeUnit {
    pub fn player() -> FakeUnit {
        FakeUnit {
            ty: UnitType::Player,
            class: 0,
            guid: 1,
            mode: 1,
            pos: Point::default(),
            size: 2,
            stats: BTreeMap::from([(10, 100 << 8), (67, 100)]),
            item_stats: BTreeMap::new(),
            states: Vec::new(),
            state_stats: BTreeMap::new(),
            used_skill: None,
            seed: Seed::default(),
            cursor: false,
            torso_speed: None,
            run_bonus: 0,
        }
    }
}

#[derive(Default)]
pub struct FakeUnits {
    pub units: BTreeMap<UnitId, FakeUnit>,
    pub walk_velocity: (i32, i32, i32),
    pub type1_expire: i32,
    pub log: Vec<String>,
    pub client_players: BTreeMap<ClientId, UnitId>,
    pub repath_budget: i32,
}

impl FakeUnits {
    pub fn with_player(unit: UnitId) -> FakeUnits {
        FakeUnits {
            units: BTreeMap::from([(unit, FakeUnit::player())]),
            walk_velocity: (6, 9, 20),
            ..FakeUnits::default()
        }
    }
    pub fn unit(&mut self, u: UnitId) -> &mut FakeUnit {
        self.units.get_mut(&u).unwrap()
    }
}

impl WalkUnits for FakeUnits {
    fn unit_type(&self, unit: UnitId) -> UnitType {
        self.units[&unit].ty
    }
    fn class(&self, unit: UnitId) -> u32 {
        self.units[&unit].class
    }
    fn guid(&self, unit: UnitId) -> u32 {
        self.units[&unit].guid
    }
    fn mode(&self, unit: UnitId) -> u32 {
        self.units[&unit].mode
    }
    fn find_unit(&self, ty: UnitType, guid: u32) -> Option<UnitId> {
        self.units
            .iter()
            .find(|(_, u)| u.ty == ty && u.guid == guid)
            .map(|(id, _)| *id)
    }
    fn position(&self, unit: UnitId) -> Point {
        self.units[&unit].pos
    }
    fn unit_size(&self, unit: UnitId) -> i32 {
        self.units[&unit].size
    }
    fn first_type1_expire(&self, _game: &Game, _unit: UnitId) -> i32 {
        self.type1_expire
    }
    fn has_cursor_item(&self, unit: UnitId) -> bool {
        self.units[&unit].cursor
    }
    fn has_state(&self, unit: UnitId, state: u16) -> bool {
        self.units[&unit].states.contains(&state)
    }
    fn state_stat(&self, unit: UnitId, state: u16, stat: u16) -> i32 {
        self.units[&unit]
            .state_stats
            .get(&(state, stat))
            .copied()
            .unwrap_or(0)
    }
    fn stat(&self, unit: UnitId, stat: u16) -> i32 {
        let u = &self.units[&unit];
        let base = u.stats.get(&stat).copied().unwrap_or(0);
        if stat == 67 {
            base + u.run_bonus
        } else {
            base
        }
    }
    fn item_stat(&self, unit: UnitId, stat: u16) -> i32 {
        self.units[&unit]
            .item_stats
            .get(&stat)
            .copied()
            .unwrap_or(0)
    }
    fn add_base_stat(&mut self, _game: &mut Game, unit: UnitId, stat: u16, delta: i32) {
        *self.unit(unit).stats.entry(stat).or_insert(0) += delta;
    }
    fn set_base_stat(&mut self, _game: &mut Game, unit: UnitId, stat: u16, value: i32) {
        self.unit(unit).stats.insert(stat, value);
    }
    fn seed(&mut self, unit: UnitId) -> &mut Seed {
        &mut self.unit(unit).seed
    }
    fn used_skill(&self, unit: UnitId) -> Option<UsedSkill> {
        self.units[&unit].used_skill
    }
    fn set_used_skill(&mut self, unit: UnitId, skill: Option<u16>) {
        self.log.push(format!("skill {skill:?}"));
        if skill.is_none() {
            self.unit(unit).used_skill = None;
        }
    }
    fn clear_queued_action(&mut self, _unit: UnitId) {
        self.log.push("clear queued".into());
    }
    fn set_mode(&mut self, _game: &mut Game, unit: UnitId, mode: u32) {
        self.log.push(format!("mode {mode}"));
        let u = self.unit(unit);
        u.mode = mode;
        // The run stat list is freed at the next mode set (§8.2).
        u.run_bonus = 0;
    }
    fn cancel_events(&mut self, _game: &mut Game, _unit: UnitId, ty: u8) {
        self.log.push(format!("cancel {ty}"));
    }
    fn schedule_event0(&mut self, _game: &mut Game, _unit: UnitId) {
        self.log.push("event0".into());
    }
    fn start_other_mode(
        &mut self,
        _game: &mut Game,
        unit: UnitId,
        mode: u32,
        _target: StartTarget,
    ) {
        self.log.push(format!("start {mode}"));
        self.unit(unit).mode = mode;
    }
    fn attach_run_stats(&mut self, _game: &mut Game, unit: UnitId, value: i32) {
        self.log.push(format!("run list {value}"));
        self.unit(unit).run_bonus = value;
    }
    fn charstats_velocity(&self, _unit: UnitId) -> (i32, i32, i32) {
        self.walk_velocity
    }
    fn torso_speed(&self, unit: UnitId) -> Option<i32> {
        self.units[&unit].torso_speed
    }
    fn client_player(&self, client: ClientId) -> Option<UnitId> {
        self.client_players.get(&client).copied()
    }
    fn send_unit_removal(&mut self, _game: &mut Game, client: ClientId, unit: UnitId) {
        self.log.push(format!("remove {} to {}", unit.0, client.0));
    }
    fn send_unit_add(&mut self, _game: &mut Game, client: ClientId, unit: UnitId) {
        self.log.push(format!("add {} to {}", unit.0, client.0));
    }
    fn repath_budget(&self, _unit: UnitId) -> i32 {
        self.repath_budget
    }
}
