// Spec: specs/drlg/levels.md §2–§5, §7, §10 (act creation, level generation, vis/warp, the waypoint room); specs/drlg/rooms.md §4.5 (streaming); specs/sim/tick.md §4 (room pass); specs/monsters/population.md §1.1, §2.1, §3, §11; specs/monsters/init.md; specs/sim/rng.md §5.2, §5.4 (game and map seeds)
//! One seed's level, built through the real path: the tables prepared
//! once ([`Prepared`], the builders of `test_fixtures::game::GameData`
//! the wired host tests use), then per seed a [`WorldSim`] over the act
//! DRLG created on the map seed (`levels.md` §3) and the game seed
//! (`rng.md` §5.2, regions `population.md` §2.1), the level generated
//! (§5), every room of it streamed in level-list order (`rooms.md` §4.5),
//! and one `d2_sim::tick::tick`, whose room pass (`tick.md` §4) runs
//! presets, objects and monster population (`population.md` §1.1) on
//! every new active room. What is left is read back: monsters (monster
//! data, path position), preset units, anchors.
//!
//! Room population (`population.md` §3) reads the act DRLG's own
//! coordinate lists, populated level and room count, warp points and
//! kind-11 location (`levels.md` §11.6), so random monsters, packs,
//! champions and uniques come from the live host. The seams without a
//! provider keep their [`WorldPending`] defaults
//! (`test_fixtures::game::Seams`). Streaming every room in level-list
//! order and populating them in one room pass is the finder's own build
//! order, not one the original produces (`levels.md` §11.6 rule 4).

use std::sync::Arc;

use anyhow::{anyhow, Result};
use d2_data::fixup::FixedSet;
use d2_data::tables::{decode_all, Levels};
use d2_formats::animdata::AnimData;
use d2_server::world_data::tables::LevelTables;
use d2_server::world_data::{Ds1Files, Dt1Files, WorldFiles};
use d2_sim::combat::vitals::VitalsTables;
use d2_sim::drlg::maze::Maze;
use d2_sim::drlg::outdoor::{SubFile, SubFiles};
use d2_sim::drlg::preset::{Ds1Input, Ds1Source};
use d2_sim::drlg::{
    room_flags, Drlg, DrlgData, DrlgRoomId, Dungeon, LevelIdx, NoLevelTypes, Services, TileInfo,
    TileRect, TileSource, TOWN_LEVELS,
};
use d2_sim::game::Game;
use d2_sim::monsters::init::GameInfo;
use d2_sim::rng::Seed;
use d2_sim::stats::StatData;
use d2_sim::units::hooks::UnitData;
use d2_sim::units::UnitType;
use d2_sim::wiring::action::{ActionHooks, ActionTables, DrlgWorld};
use d2_sim::wiring::worldgen::{
    SharedTypes, WorldPending, WorldSim, WorldState, WorldTables, WorldTypes,
};
use test_fixtures::game::{ActCreation, GameData};

use crate::query::Kind;

/// Sub-tiles per tile (`rooms.md` §9.2).
pub const SUB: i32 = 5;

/// The seams of a search game: any [`WorldPending`] host.
pub trait FinderHost: WorldPending {}

impl<X: WorldPending> FinderHost for X {}

/// A shared file map (one copy for every seed and thread).
struct Shared<T>(Arc<T>);

impl Ds1Source for Shared<Ds1Files> {
    fn ds1(&self, path: &[u8]) -> Option<&Ds1Input> {
        self.0.ds1(path)
    }
}

impl<T: SubFiles> SubFiles for Shared<T> {
    fn sub_file(&self, file: &[u8]) -> Option<&SubFile> {
        self.0.sub_file(file)
    }
}

impl TileSource for Shared<Dt1Files> {
    fn dt1(&self, path: &[u8]) -> Option<&[TileInfo]> {
        self.0.dt1(path)
    }
}

/// Everything a seed's game is built from, decoded once.
pub struct Prepared {
    pub level: LevelTables,
    ds1: Arc<Ds1Files>,
    subs: Arc<WorldFiles>,
    dt1: Arc<Dt1Files>,
    action: Arc<ActionTables>,
    world: Arc<WorldTables>,
    anim: Arc<AnimData>,
    vitals: Arc<VitalsTables>,
    stat_data: Arc<StatData>,
    unit_data: UnitData,
    pub levels: Vec<Levels>,
    pub creation: ActCreation,
}

impl Prepared {
    pub fn new(d: &GameData, creation: ActCreation) -> Result<Self> {
        let e = |e: test_fixtures::game::GameError| anyhow!("{e}");
        Ok(Prepared {
            level: d.level.clone(),
            ds1: Arc::new(d.files.ds1.clone()),
            subs: Arc::new(d.files.clone()),
            dt1: Arc::new(d.files.dt1.clone()),
            action: Arc::new(d.action_tables().map_err(e)?),
            world: Arc::new(d.world_tables().map_err(e)?),
            anim: Arc::new(d.anim.clone()),
            vitals: Arc::new(d.vitals().map_err(e)?),
            stat_data: Arc::new(d.stat_data().map_err(e)?),
            unit_data: d.unit_data().map_err(e)?,
            levels: d.rows().map_err(e)?,
            creation,
        })
    }

    /// Changes the population and init tables (a fixture's data choice;
    /// the search itself never changes a table).
    pub fn tweak_world(&mut self, f: impl FnOnce(&mut WorldTables)) {
        f(Arc::make_mut(&mut self.world));
    }

    /// The act of a levels.txt row.
    pub fn act_of(&self, level: u32) -> Option<u8> {
        self.levels
            .iter()
            .find(|l| u32::from(l.id) == level)
            .map(|l| l.act)
    }

    fn level_types(&self) -> (Arc<DrlgData>, SharedTypes) {
        let data = Arc::new(self.level.drlg.clone());
        let types = SharedTypes::new(WorldTypes::new(
            data.clone(),
            Maze::new(self.level.maze.clone()),
            self.level.preset.clone(),
            self.level.outdoor.clone(),
            Box::new(Shared(self.ds1.clone())),
            Box::new(Shared(self.subs.clone())),
        ));
        (data, types)
    }

    /// `GameData::world_sim` for any act and difficulty: the act's DRLG
    /// on the map seed (its town generated, `levels.md` §3), the action
    /// hooks on the game seed with AnimData, vitals and paths, the world
    /// state, the population regions created (`population.md` §2.1).
    pub fn world_sim<X: FinderHost>(
        &self,
        act: u8,
        difficulty: u8,
        init_seed: u32,
        game_seed: u32,
        x: X,
    ) -> Result<WorldSim<X>> {
        let (data, types) = self.level_types();
        let mut handle = types.clone();
        let town = TOWN_LEVELS[usize::from(act)];
        let drlg = match self.creation {
            ActCreation::Full => {
                Drlg::create(act, init_seed, difficulty, town, false, &data, &mut handle)
            }
            ActCreation::TownOnly => Drlg::create(
                act,
                init_seed,
                difficulty,
                0,
                false,
                &data,
                &mut NoLevelTypes,
            )
            .and_then(|mut d| {
                let l = d.get_or_alloc_level(&data, &mut handle, town)?;
                d.generate_level(&data, &mut handle, l)?;
                Ok(d)
            }),
        }
        .map_err(|e| anyhow!("act {act} creation: {e:?}"))?;
        let mut dungeon = Dungeon::default();
        dungeon.acts[usize::from(act)] = Some(drlg);
        let world = DrlgWorld {
            dungeon,
            data,
            tiles: Box::new(Shared(self.dt1.clone())),
            types: Box::new(handle),
        };
        let mut hooks = ActionHooks::new(self.action.clone(), world, Seed::init_low(game_seed), x);
        hooks.anim_data = Some(self.anim.clone());
        hooks.vitals = Some(self.vitals.clone());
        hooks.enable_paths().map_err(|e| anyhow!("paths: {e:?}"))?;
        let info = GameInfo {
            difficulty,
            expansion: true,
            ..GameInfo::default()
        };
        let state = WorldState::new(types, self.world.clone(), info);
        let mut sim = WorldSim::new(self.stat_data.clone(), self.unit_data.clone(), hooks, state);
        sim.create_regions();
        Ok(sim)
    }
}

/// A monster of the searched level after the room pass.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Monster {
    pub unit: u32,
    /// monstats row.
    pub class: u32,
    pub kind: Kind,
    pub superunique: Option<u16>,
    pub umods: Vec<u8>,
    /// Sub-tiles (path position).
    pub x: i32,
    pub y: i32,
}

/// A preset unit of the level, in absolute sub-tiles.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Preset {
    pub unit_type: u32,
    pub class: u32,
    pub x: i32,
    pub y: i32,
}

/// Where distances are measured from (sub-tiles).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Anchors {
    /// The first room (level-list order) with a warp to the `--from`
    /// level (or with any warp whose id ≠ −1 without `--from`,
    /// `levels.md` §10 step 3 `0x0066B1F0`); without one and with
    /// `--from`, the first room touching that level's rectangle. The
    /// room's centre.
    pub entrance: Option<(i32, i32)>,
    /// The waypoint of the waypoint room (`levels.md` §10.4), else that
    /// room's centre.
    pub waypoint: Option<(i32, i32)>,
}

/// What one seed's level holds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LevelView {
    pub level: u32,
    pub rooms: usize,
    pub anchors: Anchors,
    pub presets: Vec<Preset>,
    pub monsters: Vec<Monster>,
}

fn centre(r: TileRect) -> (i32, i32) {
    ((r.x * 2 + r.w) * SUB / 2, (r.y * 2 + r.h) * SUB / 2)
}

/// Rectangles sharing an edge or overlapping.
fn touches(a: TileRect, b: TileRect) -> bool {
    a.x <= b.x + b.w && b.x <= a.x + a.w && a.y <= b.y + b.h && b.y <= a.y + a.h
}

fn anchors(
    d: &mut Drlg,
    svc: &mut Services<'_>,
    l: LevelIdx,
    level: u32,
    from: Option<u32>,
) -> Result<Anchors, d2_sim::drlg::DrlgError> {
    let rooms = d.level_rooms(l);
    let vis = d.vis_array(svc.data, level)?;
    let warp = d.warp_array(svc.data, level)?;
    let warp_to = |f: u32, i: usize| {
        f & (room_flags::WARP_0 << i) != 0 && warp[i] != -1 && from.is_none_or(|x| vis[i] == x)
    };
    let mut entrance = rooms
        .iter()
        .find(|&&r| (0..8).any(|i| warp_to(d.room(r).flags, i)))
        .map(|&r| centre(d.room(r).rect));
    if entrance.is_none() {
        if let Some(f) = from.and_then(|f| d.find_level(f)) {
            let fr = d.level(f).rect;
            entrance = rooms
                .iter()
                .find(|&&r| touches(d.room(r).rect, fr))
                .map(|&r| centre(d.room(r).rect));
        }
    }
    let mut waypoint = None;
    if let Some(&r) = rooms
        .iter()
        .find(|&&r| d.room(r).flags & room_flags::ANY_WAYPOINT != 0)
    {
        let rect = d.room(r).rect;
        waypoint = Some(
            svc.types
                .preset_units(d, r)
                .into_iter()
                .find(|u| u.unit_type == 2 && u.class < 573 && svc.data.is_waypoint_object(u.class))
                .map_or(centre(rect), |u| (rect.x * SUB + u.x, rect.y * SUB + u.y)),
        );
    }
    Ok(Anchors { entrance, waypoint })
}

/// Builds the level of `level` for one seed pair and reads it back.
/// An `Err` is a step the d2rs path cannot take for this level (DRLG,
/// level type, population or init error): "unsupported", never a guess.
pub fn build_level<X: FinderHost>(
    p: &Prepared,
    level: u32,
    difficulty: u8,
    from: Option<u32>,
    init_seed: u32,
    game_seed: u32,
    x: X,
) -> Result<LevelView> {
    let act = p
        .act_of(level)
        .ok_or_else(|| anyhow!("level {level}: no levels.txt row"))?;
    if act as usize >= TOWN_LEVELS.len() {
        return Err(anyhow!("level {level}: act {act} out of range"));
    }
    let mut sim = p.world_sim(act, difficulty, init_seed, game_seed, x)?;
    let mut game = Game::new();
    game.lists
        .ensure_act(act)
        .map_err(|e| anyhow!("act list: {e:?}"))?;
    type Built = (usize, Anchors, Vec<Preset>);
    let built: Built = sim
        .action
        .sys
        .hooks
        .drlg
        .with_act(act, &mut game.lists, |d, svc| {
            let l = d.get_or_alloc_level(svc.data, svc.types, level)?;
            if d.level_rooms(l).is_empty() {
                d.generate_level(svc.data, svc.types, l)?;
            }
            let rooms: Vec<DrlgRoomId> = d.level_rooms(l);
            for &r in &rooms {
                d.stream_room(svc, r)?;
            }
            let mut presets = Vec::new();
            for &r in &rooms {
                let rect = d.room(r).rect;
                for u in svc.types.preset_units(d, r) {
                    presets.push(Preset {
                        unit_type: u.unit_type,
                        class: u.class,
                        x: rect.x * SUB + u.x,
                        y: rect.y * SUB + u.y,
                    });
                }
            }
            let a = anchors(d, svc, l, level, from)?;
            Ok::<_, d2_sim::drlg::DrlgError>((rooms.len(), a, presets))
        })
        .ok_or_else(|| anyhow!("act {act} has no DRLG"))?
        .map_err(|e| anyhow!("level {level}: {e:?}"))?;
    let (rooms, anchors, presets) = built;
    // The room pass of the first tick populates every new active room.
    d2_sim::tick::tick(&mut game, &mut sim);
    let errors = sim.errors();
    if !errors.is_empty() {
        return Err(anyhow!("level {level}: {}", errors.join("; ")));
    }
    let mut units = game.lists.units_of_type(UnitType::Monster);
    units.sort();
    let mut monsters = Vec::new();
    for u in units {
        let Some(md) = sim.world.monsters.get(u) else {
            continue;
        };
        if md.level_id != level as i32 {
            continue;
        }
        let kind = Kind::from_flags(md.type_flags);
        let (x, y) = sim.action.sys.hooks.path_position(u);
        monsters.push(Monster {
            unit: u.0,
            class: md.class,
            kind,
            superunique: (kind == Kind::SuperUnique).then_some(md.boss_hc_idx),
            umods: md.umod_list().to_vec(),
            x,
            y,
        });
    }
    Ok(LevelView {
        level,
        rooms,
        anchors,
        presets,
        monsters,
    })
}

/// monstats `Id` per row from the text table when the install has it
/// (`None` per row otherwise, and when the row count differs from the
/// loaded table's).
pub fn monster_names(set: &d2_formats::mpq::ArchiveSet, fixed: &FixedSet) -> Vec<Option<String>> {
    let rows = fixed.table("monstats").map_or(0, |t| t.iter().count());
    let none = vec![None; rows];
    let Ok(Some((_, bytes))) = d2_data::bin::read_excel(set, "monstats.txt") else {
        return none;
    };
    let Ok(t) = d2_data::txt::TxtTable::parse("monstats.txt", &bytes) else {
        return none;
    };
    let names: Vec<Option<String>> = t
        .records
        .iter()
        .map(|r| {
            r.cells
                .first()
                .map(|c| String::from_utf8_lossy(c).into_owned())
        })
        .collect();
    if names.len() == rows {
        names
    } else {
        none
    }
}

/// Typed rows of a fixed-up table (used for level names).
pub fn level_rows(fixed: &FixedSet) -> Vec<Levels> {
    fixed
        .table("levels")
        .and_then(|t| decode_all(t).ok())
        .unwrap_or_default()
}
