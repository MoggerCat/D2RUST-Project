//! The item-move handlers through the real host frame (drain → handle →
//! tick → flush) on the wired host: `SimGame<ActionSim, WiredWorld>`
//! whose [`InvParts`] hold the inventory model on synthetic tables (the
//! inventory wiring's fixture shape: a barbarian, record 4, 10 × 4), the
//! real `d2-proto` sizes, real item units from `Economy::create_item` on
//! the action sim's own unit records, stat lists and item store, in a
//! real field room. Only [`MoveRest`] (the seams no d2-sim module
//! provides) is a fake: it answers what the test sets and logs.
//!
//! The bytes are exact: the 0x9C / 0x9D headers of §11 (each item bit
//! stream is decoded with `d2-proto`'s reader and checked against the
//! item, then cut off: `T::streams`), 0x47 / 0x48 after each update
//! pass, and the direct sends.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};

use d2_data::bin::BinTable;
use d2_data::fixup::maps::EquivMatrix;
use d2_data::fixup::records::stat_ops;
use d2_data::tables::{Itemratio, Itemstatcost, Itemtypes, Record, States};
use d2_sim::combat::CombatTables;
use d2_sim::game::Game;
use d2_sim::items::inventory::tables::{GridRec, InvItemRec, InvTypeRec};
use d2_sim::items::inventory::{InvItem, UnitKind};
use d2_sim::items::moves::{stat, ty, Guid, MovePending, Spot};
use d2_sim::items::tables::ItemRec;
use d2_sim::items::{q, ItemRequest, ItemTables};
use d2_sim::rng::Seed;
use d2_sim::skills::SkillTables;
use d2_sim::stats::{ClassStats, StatData, StatTable, StateTable};
use d2_sim::units::hooks::UnitData;
use d2_sim::units::lifecycle::AllocRequest;
use d2_sim::units::lists::client_state;
use d2_sim::units::messages::update_item_stat;
use d2_sim::units::{RoomId, UnitType};
use d2_sim::wiring::action::{ActionHooks, ActionSim, ActionTables};
use d2_sim::wiring::economy::{GameFields, ItemSpawn};
use d2_sim::world::npc::NpcControl;
use d2_sim::world::quests::{QuestControl, QuestTables};
use d2_sim::world::vendors::VendorTables;

use super::*;
use crate::adapters::handlers::items::ITEM_IDS;
use crate::adapters::handlers::world::tests::trade_quests::{ActionRest, Rest};
use crate::adapters::handlers::world::tests::waypoints::{field_drlg, field_room};
use crate::adapters::handlers::world::{ActionEvents, ActionWorld, WiredWorld};
use crate::adapters::item_bits::TablesLookup;
use crate::adapters::{PlayerData, PlayerFields, ProtoSizes};
use crate::dispatch::Outcome;
use crate::host::{Handled, Host};
use crate::seams::{Clock, PlayerGate, SessionHandler};
use d2_proto::item_bits::decode;

const N_STATS: usize = 359;
const N_TYPES: usize = 80;
const GAME_SEED: u32 = 0x1A7E;
/// Barbarian: inventory record 4 (10 × 4, §1.3).
const CLASS: u32 = 4;

// Item records (combined index).
const CAP: usize = 0;
const GOLD: usize = 1;
const SWORD: usize = 2;
const TWO_HANDER: usize = 3;
const SHIELD: usize = 4;
const HP1: usize = 5;
const KEY: usize = 6;
const TSC: usize = 7;
const TBK: usize = 8;
const SCROLL: usize = 7;
const BOOK: usize = 8;

const T_SWOR: u16 = ty::SWOR;
const T_SHIE: u16 = ty::SHIE;
const T_HELM: u16 = ty::HELM;
const T_HPOT: u16 = ty::HPOT;
const T_GOLD: u16 = ty::GOLD;
const T_KEY: u16 = 41;
const T_WEAP: u16 = 45;
const T_ARMO: u16 = 50;
const T_MISC: u16 = 52;

/// (code, type, invwidth, invheight, autobelt, useable, stackable,
/// maxstack, durability) per record.
type Row = ([u8; 4], u16, u8, u8, u8, u8, u8, u32, u8);
const ROWS: [Row; 9] = [
    (*b"cap ", T_HELM, 2, 2, 0, 0, 0, 0, 12),
    (*b"gld ", T_GOLD, 1, 1, 0, 0, 0, 0, 0),
    (*b"ssd ", T_SWOR, 1, 3, 0, 0, 0, 0, 24),
    (*b"2hs ", T_SWOR, 2, 4, 0, 0, 0, 0, 44),
    (*b"buc ", T_SHIE, 2, 2, 0, 0, 0, 0, 12),
    (*b"hp1 ", T_HPOT, 1, 1, 1, 1, 0, 0, 0),
    (*b"key ", T_KEY, 1, 1, 0, 0, 1, 12, 0),
    (*b"tsc ", ty::SCRO, 1, 1, 0, 1, 0, 0, 0),
    (*b"tbk ", ty::BOOK, 1, 2, 0, 1, 1, 20, 0),
];

fn set_u16(r: &mut [u8], o: usize, v: u16) {
    r[o..o + 2].copy_from_slice(&v.to_le_bytes());
}

pub(crate) fn stat_data() -> Arc<StatData> {
    let size = Itemstatcost::SIZE;
    let mut records = vec![0u8; N_STATS * size];
    for s in 0..N_STATS {
        let r = &mut records[s * size..(s + 1) * size];
        for o in [0x32, 0x48, 0x4A, 0x56, 0x58, 0x5A, 0x5C] {
            set_u16(r, o, 0xFFFF);
        }
        set_u16(r, 0, s as u16);
    }
    let mut t = BinTable {
        name: "itemstatcost".into(),
        source: "synthetic".into(),
        count: N_STATS,
        record_size: size,
        records,
    };
    stat_ops(&mut t);
    // 200 empty states: the potion states 100 / 106 exist
    // (`items/use.md` §3.1 step 1).
    let states = BinTable {
        name: "states".into(),
        source: "synthetic".into(),
        count: 200,
        record_size: States::SIZE,
        records: vec![0; 200 * States::SIZE],
    };
    let maps = d2_data::fixup::maps::states(&states);
    Arc::new(StatData {
        stats: StatTable::from_fixed(&t).expect("itemstatcost"),
        classes: vec![ClassStats::default(); 7],
        states: StateTable::new(&states, &maps).expect("states"),
        damage_regen: vec![0; 8],
        aurastate: vec![0; 8],
        rescale_precision: d2_sim::stats::DEFAULT_RESCALE_PRECISION,
    })
}

fn equiv() -> EquivMatrix {
    let n = N_TYPES;
    let words = n.div_ceil(32);
    let mut m = EquivMatrix {
        n,
        words,
        bits: vec![0; n * words],
    };
    let mut set = |i: usize, j: usize| m.bits[i * words + j / 32] |= 1 << (j % 32);
    for i in 1..n {
        set(i, 0);
        set(i, i);
    }
    for (c, p) in [
        (T_HELM, T_ARMO),
        (T_SHIE, T_ARMO),
        (T_SWOR, T_WEAP),
        (T_GOLD, T_MISC),
        (T_HPOT, T_MISC),
        (T_KEY, T_MISC),
        (ty::SCRO, T_MISC),
        (ty::BOOK, T_MISC),
    ] {
        set(usize::from(c), usize::from(p));
    }
    m
}

fn item_tables() -> ItemTables {
    let mut ratio = Itemratio::decode(&[0u8; Itemratio::SIZE]);
    ratio.version = 1;
    let itemtypes = (0..N_TYPES)
        .map(|_| {
            let mut t = Itemtypes::decode(&[0u8; Itemtypes::SIZE]);
            t.class = 0xFF;
            t.staffmods = 0xFF;
            // Empty `shoots`: the link miss (link16 −1).
            t.shoots = 0xFFFF;
            t.rare = 1;
            t
        })
        .collect();
    ItemTables {
        items: ROWS
            .iter()
            .map(|r| ItemRec {
                code: r.0,
                type_: r.1 as i16,
                level: 1,
                invwidth: r.2,
                invheight: r.3,
                stackable: r.6,
                maxstack: r.7,
                durability: r.8,
                ..ItemRec::default()
            })
            .collect(),
        itemtypes,
        equiv: equiv(),
        itemratio: vec![ratio],
        valshift: vec![0; N_STATS],
        stat_shift: 6,
        stat_mask: 0x3F,
        ..ItemTables::default()
    }
}

/// The measured grid records of §1.3 (0–15) and the belt boxes.
fn inv_tables() -> InvTables {
    let g = |x, y| GridRec {
        grid_x: x,
        grid_y: y,
    };
    let mut grids = vec![g(10, 4); 16];
    grids[5] = g(10, 10);
    grids[8] = g(6, 4);
    grids[9] = g(3, 4);
    grids[12] = g(6, 8);
    grids[13] = g(0, 0);
    let mut itemtypes = vec![
        InvTypeRec {
            class: 7,
            ..InvTypeRec::default()
        };
        N_TYPES
    ];
    for (t, loc1, loc2) in [(T_HELM, 1, 1), (T_SWOR, 4, 5), (T_SHIE, 5, 4)] {
        let r = &mut itemtypes[usize::from(t)];
        r.body = 1;
        r.bodyloc1 = loc1;
        r.bodyloc2 = loc2;
    }
    itemtypes[usize::from(T_HPOT)].beltable = 1;
    InvTables {
        grids,
        belts: vec![12, 8, 4, 16, 8, 12, 16, 12, 8, 4, 16, 8, 12, 16],
        items: ROWS
            .iter()
            .map(|r| InvItemRec {
                code: r.0,
                type_: r.1 as i16,
                invwidth: r.2,
                invheight: r.3,
                autobelt: r.4,
                useable: r.5,
                stackable: r.6,
                maxstack: r.7,
                // `pSpell` of the live `misc.txt` rows (`items/use.md` §3).
                pspell: match &r.0 {
                    b"tsc " | b"tbk " => 2,
                    b"hp1 " => 3,
                    _ => 0,
                },
                // The live `hp1` use fields (`items/use.md` §3.1): state
                // 100, stat 74, `calc1` 30 at 0, `len` 192 at 3.
                use_state: if &r.0 == b"hp1 " { 100 } else { 0 },
                use_stat: [if &r.0 == b"hp1 " { 74 } else { -1 }, -1, -1],
                use_calc: [0, u32::MAX, u32::MAX],
                use_len: 3,
                ..InvItemRec::default()
            })
            .collect(),
        itemtypes,
        equiv: equiv(),
        books: Vec::new(),
        item_use: d2_sim::items::inventory::ItemUseTables {
            code: vec![0x07, 30, 0x00, 0x08, 192, 0, 0x00],
            maxstat: Vec::new(),
        },
    }
}

/// A normal-quality request for a record, never ethereal.
fn plain(record: usize) -> ItemRequest {
    ItemRequest {
        item: record as i32,
        format: 101,
        ilvl: 1,
        quality: q::NORMAL,
        flags2: 0x2,
        ..ItemRequest::default()
    }
}

/// The answers of the [`MoveRest`] fake and its call log.
#[derive(Default)]
pub(crate) struct RestState {
    pub(crate) log: Vec<String>,
    pub(crate) pos: BTreeMap<Owner, (i32, i32)>,
    pub(crate) distance: i32,
    pub(crate) spot: Option<Spot>,
    pub(crate) room_at: bool,
    pub(crate) in_town: bool,
    pub(crate) two_handed: BTreeSet<Guid>,
    pub(crate) use_ok: bool,
    pub(crate) gold: bool,
    pub(crate) spells: BTreeMap<Guid, i32>,
    pub(crate) sent: Vec<(Owner, Vec<u8>)>,
    /// The player has no used skill (`0x00620250` = null; §7.23 rule 2).
    pub(crate) no_used_skill: bool,
}

/// [`MoveRest`] fake, shared with the test (and the cube and vendor
/// tests, whose inventories are the same model).
#[derive(Clone, Default)]
pub(crate) struct MRest(Arc<Mutex<RestState>>);

impl MRest {
    pub(crate) fn with<T>(&self, f: impl FnOnce(&mut RestState) -> T) -> T {
        f(&mut self.0.lock().unwrap())
    }
    fn log(&self, s: String) {
        self.with(|r| r.log.push(s));
    }
    pub(crate) fn take_log(&self) -> Vec<String> {
        self.with(|r| std::mem::take(&mut r.log))
    }
}

impl MovePending for MRest {
    fn has_used_skill(&self, _: Owner) -> bool {
        !self.with(|r| r.no_used_skill)
    }
    fn distance(&self, _: Owner, _: Owner) -> i32 {
        self.with(|r| r.distance)
    }
    fn walk_to_item(&mut self, player: Owner, item: Guid, cursor: bool) {
        self.log(format!("walk_to_item {} {item} {cursor}", player.guid));
    }
    fn room_at(&self, _: i32, _: i32) -> bool {
        self.with(|r| r.room_at)
    }
    fn free_spot(
        &self,
        _: (i32, i32),
        _: (i32, i32),
        _: u32,
        _: u32,
        _: u32,
        _: u32,
    ) -> Option<Spot> {
        self.with(|r| r.spot)
    }
    fn in_town(&self, _: Owner) -> bool {
        self.with(|r| r.in_town)
    }
    fn sound(&mut self, u: Owner, id: u32) {
        self.log(format!("sound {} {id:#x}", u.guid));
    }
    fn pickup_sound(&mut self, player: Owner, item: Guid) {
        self.log(format!("pickup_sound {} {item}", player.guid));
    }
    fn quest_item_dropped(&mut self, item: Guid) {
        self.log(format!("quest_item_dropped {item}"));
    }
    fn set_owner(&mut self, item: Guid, owner: Owner) {
        self.log(format!("set_owner {item} {}", owner.guid));
    }
    fn use_item(&mut self, player: Owner, target: Owner, item: Guid) -> bool {
        self.log(format!("use_item {} {} {item}", player.guid, target.guid));
        self.with(|r| r.use_ok)
    }
    fn send(&mut self, player: Owner, bytes: Vec<u8>) {
        self.with(|r| r.sent.push((player, bytes)));
    }
}

impl InvRest for MRest {
    /// Every player has a left and a right skill (Attack, skill 0, class
    /// entry, at the least): the weapon bookkeeping reads them once a
    /// weapon is in use (`inventory.md` §5.8 step 5).
    fn mouse_skill(&self, _: Owner, _: bool) -> Option<(i32, i32)> {
        Some((0, -1))
    }
    fn pos(&self, u: Owner) -> (i32, i32) {
        self.with(|r| r.pos.get(&u).copied().unwrap_or((0, 0)))
    }
    fn set_pos(&mut self, u: Owner, x: i32, y: i32) {
        self.with(|r| r.pos.insert(u, (x, y)));
    }
    fn gold_request(&self, _: Owner, gld: usize) -> Option<(ItemRequest, ItemSpawn)> {
        self.with(|r| r.gold).then(|| {
            (
                plain(gld),
                ItemSpawn {
                    room: None,
                    mode: 3,
                    init_flags: 1,
                },
            )
        })
    }
    fn spell(&self, item: Guid) -> i32 {
        self.with(|r| r.spells.get(&item).copied().unwrap_or(0))
    }
    fn item_active_on(&self, _: Guid, _: Owner) -> bool {
        false
    }
    fn own_contribution(&self, _: Guid, _: Owner, _: u16) -> i32 {
        0
    }
    fn level_requirement(&self, _: Guid, _: Owner) -> i32 {
        -1
    }
    fn two_handed(&self, item: Guid) -> bool {
        self.with(|r| r.two_handed.contains(&item))
    }
    fn one_or_two_handed(&self, _: Owner, _: Guid) -> bool {
        false
    }
    fn ammo_type(&self, _: Guid) -> Option<i16> {
        None
    }
    fn has_allowed_location(&self, _: Guid) -> bool {
        true
    }
    fn quiver_kind(&self, _: Guid) -> bool {
        false
    }
    fn player_data_4c(&self, _: Owner) -> u32 {
        0
    }
    fn player_data_50(&self, _: Owner) -> u32 {
        0
    }
    fn npc_talking(&self, _: Owner, _: Owner) -> bool {
        false
    }
    fn player_trade_gate(&self, _: Owner) -> Option<bool> {
        None
    }
}

impl MoveRest for MRest {
    fn take_sent(&mut self) -> Vec<(Owner, Vec<u8>)> {
        self.with(|r| std::mem::take(&mut r.sent))
    }
}

#[derive(Default)]
struct Session;

impl SessionHandler for Session {
    fn system_message(&mut self, _: ClientId, _: &[u8], _: usize, _: &mut dyn MessageSink) {}
}

struct Manual(u32);

impl Clock for Manual {
    fn now_ms(&mut self) -> u32 {
        self.0
    }
}

type Sim = SimGame<ActionSim<ActionRest>, WiredWorld<Rest>>;
type TestHost = Host<Sim, ProtoSizes, Session, Manual>;

const ALIVE: PlayerFields = PlayerFields {
    gate: PlayerGate {
        mode: 1,
        uninterruptable: false,
    },
    data: Some(PlayerData { last_accept: 0 }),
};

fn action_tables() -> ActionTables {
    ActionTables {
        missiles: Vec::new(),
        skills: SkillTables {
            skills: Vec::new(),
            skilldesc: Vec::new(),
            missiles: Vec::new(),
            skills_code: Vec::new(),
            miss_code: Vec::new(),
            level_cap: 0,
            stat_count: 0,
        },
        combat: CombatTables {
            charstats: Vec::new(),
            difficultylevels: Vec::new(),
            monstats: Vec::new(),
            monstats2: Vec::new(),
            hitclass: Vec::new(),
        },
        levels: Vec::new(),
        skill_modes: Vec::new(),
        overlay_count: 0,
        monequip: Vec::new(),
        arena: Vec::new(),
    }
}

/// A host for client 0: game creation on the action sim (`expansion`,
/// the game seed), the wired host with the inventory parts, a barbarian
/// in a generated field room with an inventory, level 1, strength and
/// dexterity 10, at (10, 10); the client joined in the player's room.
/// One frame has run.
struct T {
    host: TestHost,
    player: UnitId,
    room: RoomId,
    rest: MRest,
}

fn setup() -> T {
    setup_with(true)
}

fn setup_with(expansion: bool) -> T {
    let rest = MRest::default();
    rest.with(|r| r.distance = 1);
    let boxed = Box::new(rest.clone());
    build(expansion, rest, boxed)
}

/// [`setup_with`] over `PreviewMoveRest` (the play host's rest): the
/// player's place comes from the staged path position.
fn setup_preview() -> T {
    let rest = MRest::default();
    let boxed = Box::new(crate::adapters::handlers::world::PreviewMoveRest::default());
    build(true, rest, boxed)
}

fn build(expansion: bool, rest: MRest, boxed: Box<dyn MoveRest + Send + Sync>) -> T {
    let hooks = ActionHooks::new(
        Arc::new(action_tables()),
        field_drlg(),
        Seed::init(),
        ActionRest::default(),
    );
    let data = UnitData {
        expansion,
        ..UnitData::default()
    };
    let mut events = ActionSim::new(stat_data(), data, hooks);
    events.create_game(&GameFields::new(Seed::init_low(GAME_SEED), expansion));
    let mut game = Game::new();
    let room = field_room(&mut events, &mut game);
    let mut seed = events.hooks().game_seed;
    let npc = NpcControl::new(&[], Vec::new(), expansion, 0, &mut seed).unwrap();
    let quests = QuestControl::new(&QuestTables::load().unwrap(), &mut seed).unwrap();
    events.hooks().game_seed = seed;
    let mut world = WiredWorld::new(
        ActionWorld::default(),
        item_tables(),
        quests,
        npc,
        VendorTables::default(),
        Rest::default(),
        1000,
    );
    world.inventory = Some(InvParts::new(inv_tables(), boxed));
    let req = AllocRequest {
        ty: UnitType::Player,
        class: CLASS,
        room: Some(room),
        add: true,
        fixed_guid: None,
        mode: 1,
        allied: false,
    };
    let player = events
        .with(&mut game, |g, v| v.allocate(g, &req, 10, 10))
        .unwrap();
    let mut sim: Sim = SimGame::with_world(game, events, world);
    let guid = sim.events.sys.units.get(player).unwrap().guid;
    let inv = sim.world.inventory.as_mut().unwrap();
    inv.state
        .add_inventory(player, UnitKind::Player { class: CLASS as u8 }, guid);
    rest.with(|r| r.pos.insert(Owner::player(guid), (10, 10)));
    sim.join(0, Some(player), Some(room), client_state::IN_GAME)
        .unwrap();
    sim.set_player(player, ALIVE);
    let mut host = Host::new(sim, ProtoSizes, Session, Manual(1000));
    host.connect(0);
    host.frame().unwrap();
    let mut t = T {
        host,
        player,
        room,
        rest,
    };
    t.set_stat(player, stat::LEVEL, 1);
    t.set_stat(player, 0, 10);
    t.set_stat(player, 2, 10);
    t
}

impl T {
    fn sim(&mut self) -> &mut Sim {
        &mut self.host.game
    }
    fn inv(&mut self) -> &mut InvParts {
        self.sim().world.inventory.as_mut().unwrap()
    }
    fn set_stat(&mut self, u: UnitId, s: u16, v: i32) {
        let sys = &mut self.sim().events.sys;
        sys.stats.unit_set(&mut sys.hooks, u, s, v, 0);
    }
    fn stat(&mut self, u: UnitId, s: u16) -> i32 {
        self.sim().events.sys.stats.unit_total(u, s, 0)
    }
    fn pguid(&mut self) -> Guid {
        let p = self.player;
        self.sim().events.sys.units.get(p).unwrap().guid
    }
    fn unit(&mut self, g: Guid) -> Option<UnitId> {
        self.sim().game.lists.find_unit(UnitType::Item, g)
    }
    fn mode(&mut self, g: Guid) -> u32 {
        let u = self.unit(g).unwrap();
        self.sim().events.sys.units.get(u).unwrap().mode
    }
    fn data(&mut self, g: Guid) -> InvItem {
        let u = self.unit(g).unwrap();
        self.inv().state.items[&u]
    }
    fn inventory(&mut self) -> &d2_sim::items::inventory::Inventory {
        let p = self.player;
        &self.inv().state.inventories[&p]
    }
    fn in_room(&mut self, g: Guid) -> bool {
        let u = self.unit(g).unwrap();
        self.sim().game.lists.unit(u).unwrap().room().is_some()
    }

    /// A normal item of `record` created on the action sim's units into
    /// the host's item store, on the ground of the room at (x, y),
    /// identified (callers set 0x10, `generation.md` §1.4).
    fn ground_item(&mut self, record: usize, x: i32, y: i32) -> Guid {
        let room = self.room;
        let sim = &mut self.host.game;
        let (game, events, world) = (&mut sim.game, &mut sim.events, &mut sim.world);
        let u = world
            .with_economy(game, events, |econ, _| {
                econ.create_item(
                    &mut plain(record),
                    false,
                    ItemSpawn {
                        room: Some(room),
                        mode: 3,
                        init_flags: 1,
                    },
                )
            })
            .unwrap();
        events.sys.hooks.items.get_mut(u).unwrap().flags |= 0x10;
        let guid = events.sys.units.get(u).unwrap().guid;
        // The item's position is its item data's (§2.2, §9.1 step 3).
        let inv = world.inventory.as_mut().unwrap();
        inv.state.items.insert(
            u,
            InvItem {
                x,
                y,
                ..InvItem::new(guid, record)
            },
        );
        guid
    }

    /// A ground item picked to the cursor through 0x16 (one frame).
    fn cursor_item(&mut self, record: usize) -> Guid {
        let g = self.ground_item(record, 11, 11);
        assert_eq!(self.frame(&pick(g, 1)).0, ResultCode::Done);
        g
    }

    /// A ground item auto-picked through 0x16 (one frame).
    fn picked(&mut self, record: usize) -> Guid {
        let g = self.ground_item(record, 11, 11);
        assert_eq!(self.frame(&pick(g, 0)).0, ResultCode::Done);
        g
    }

    /// One frame (drain → handle → tick → flush) with `msg` sent: the
    /// dispatch result and the bytes client 0 receives, in order. Frames
    /// are 240 ms apart, past the client's 200 ms duplicate filter.
    fn frame(&mut self, msg: &[u8]) -> (ResultCode, Vec<Vec<u8>>) {
        let r = self.frame_raw(msg);
        self.no_errors();
        r
    }

    /// [`Self::frame`] without the error checks.
    fn frame_raw(&mut self, msg: &[u8]) -> (ResultCode, Vec<Vec<u8>>) {
        self.host.clock.0 += 240;
        self.host.send_game(0, msg).unwrap();
        let r = self.host.frame().unwrap();
        assert!(r.ticked);
        assert_eq!(r.messages.len(), 1, "{:?}", r.messages);
        let Handled::Game(Outcome::Dispatched(code)) = r.messages[0].handled else {
            panic!("{:?}", r.messages[0]);
        };
        let got = self.host.receive(0);
        (code, self.streams(got))
    }

    /// Checks the item bit stream of each 0x9C / 0x9D
    /// (`items/bitstream.md`) and returns the messages with it cut off
    /// (size byte = header size), so the tests state the headers of §11.
    /// The stream must decode to its exact length with `d2-proto`'s
    /// reader on the host's tables (`adapters::item_bits`), carry the
    /// item's code and, for an item still in the game, its mode in the
    /// item's last message of the batch (a message sent "now", §6.4,
    /// carries the mode of its moment).
    fn streams(&mut self, msgs: Vec<Vec<u8>>) -> Vec<Vec<u8>> {
        let guid_of = |m: &[u8]| u32::from_le_bytes(m[4..8].try_into().unwrap());
        let last: BTreeMap<Guid, usize> = msgs
            .iter()
            .enumerate()
            .filter(|(_, m)| m[0] == 0x9C || m[0] == 0x9D)
            .map(|(i, m)| (guid_of(m), i))
            .collect();
        msgs.into_iter()
            .enumerate()
            .map(|(i, mut m)| {
                let head = match m[0] {
                    0x9C => 8,
                    0x9D => 13,
                    _ => return m,
                };
                assert_eq!(usize::from(m[2]), m.len(), "size byte {m:?}");
                let guid = guid_of(&m);
                let bits = {
                    let tables = &self.sim().world.tables;
                    decode(&m[head..], &TablesLookup(tables))
                        .unwrap_or_else(|e| panic!("stream of {m:?}: {e}"))
                };
                if let Some(u) = self.unit(guid) {
                    let record = self.sim().events.sys.hooks.items.get(u).unwrap().record;
                    assert_eq!(bits.code, ROWS[record].0, "code of {guid}");
                    if last[&guid] == i {
                        assert_eq!(u32::from(bits.mode), self.mode(guid), "mode of {guid}");
                    }
                }
                m.truncate(head);
                m[2] = head as u8;
                m
            })
            .collect()
    }

    /// A frame without a message (the next tick's update pass).
    fn idle(&mut self) -> Vec<Vec<u8>> {
        self.host.clock.0 += 240;
        let r = self.host.frame().unwrap();
        assert!(r.ticked);
        self.no_errors();
        let got = self.host.receive(0);
        self.streams(got)
    }

    fn no_errors(&mut self) {
        let sim = self.sim();
        if let Some(inv) = &sim.world.inventory {
            assert_eq!(inv.state.errors, Vec::new());
        }
        assert!(
            sim.events.sys.hooks.errors.is_empty(),
            "{:?}",
            sim.events.sys.hooks.errors
        );
        assert_eq!(sim.tick_faults, Vec::new());
        assert_eq!(sim.world.action.faults, Vec::new());
    }

    /// The update pass's 0x47 and 0x48 for the player (§11).
    fn relators(&mut self) -> [Vec<u8>; 2] {
        let g = self.pguid();
        let mut a = vec![0x47, 0, 0];
        a.extend_from_slice(&g.to_le_bytes());
        a.extend_from_slice(&[0; 4]);
        let mut b = a.clone();
        b[0] = 0x48;
        [a, b]
    }

    /// An update pass's bytes: the item messages, then 0x47, 0x48.
    fn pass(&mut self, items: &[Vec<u8>]) -> Vec<Vec<u8>> {
        let mut v = items.to_vec();
        v.extend(self.relators());
        v
    }

    /// [`Self::pass`] after a pick-up: the pickup sound on the player
    /// (S→C 0x2C, event 1; REC-1404) closes the tick's messages.
    fn pass_pick(&mut self, items: &[Vec<u8>]) -> Vec<Vec<u8>> {
        let mut v = self.pass(items);
        let g = self.pguid();
        let mut m = vec![0x2C, 0];
        m.extend_from_slice(&g.to_le_bytes());
        m.extend_from_slice(&1u16.to_le_bytes());
        v.push(m);
        v
    }

    /// 0x9D for an item the player owns (§11: owner type 0).
    fn owned(&mut self, action: u8, item: Guid) -> Vec<u8> {
        let p = self.pguid();
        x9d(action, item, 0, p)
    }
}

/// 0x9C header (§11; stream cut off by `T::streams`): [id, action,
/// size 8, category 0, GUID].
fn x9c(action: u8, item: Guid) -> Vec<u8> {
    let mut b = vec![0x9C, action, 8, 0];
    b.extend_from_slice(&item.to_le_bytes());
    b
}

/// 0x9D header (§11; stream cut off by `T::streams`): size 13,
/// category 0, owner.
fn x9d(action: u8, item: Guid, owner_type: u8, owner: Guid) -> Vec<u8> {
    let mut b = vec![0x9D, action, 13, 0];
    b.extend_from_slice(&item.to_le_bytes());
    b.push(owner_type);
    b.extend_from_slice(&owner.to_le_bytes());
    b
}

// ---- C→S messages (`client-messages.tsv` layouts) ------------------------

fn msg(id: u8, fields: &[u32]) -> Vec<u8> {
    let mut m = vec![id];
    for f in fields {
        m.extend_from_slice(&f.to_le_bytes());
    }
    m
}
/// 0x16 [type 4][GUID][cursor].
fn pick(item: Guid, cursor: u32) -> Vec<u8> {
    msg(0x16, &[4, item, cursor])
}
/// 0x1A / 0x1B / 0x1D / 0x1E: [item u32][location u8], 9 bytes.
fn body(id: u8, item: Guid, loc: u32) -> Vec<u8> {
    msg(id, &[item, loc])
}
/// 0x1C / 0x61: [location u16].
fn loc16(id: u8, loc: u16) -> Vec<u8> {
    let mut m = vec![id];
    m.extend_from_slice(&loc.to_le_bytes());
    m
}

const NO_BYTES: Vec<Vec<u8>> = Vec::new();
use ResultCode::{Done, Invalid, Malformed, Refused};

// ---- the id tables ----------------------------------------------------------------------

const CLIENT_TSV: &str = include_str!("../../../../../../../specs/sim/client-messages.tsv");

/// Rows of [`MOVE_IDS`] that disagree with `client-messages.tsv` (name,
/// `transport_size`, kind `handler`) or with `items::moves::HANDLED` (the
/// size), and the `HANDLED` ids missing from the table (M05).
fn ids_vs_tsv(tsv: &str) -> Vec<String> {
    let rows: BTreeMap<u8, Vec<String>> = tsv
        .lines()
        .skip(1)
        .filter_map(|l| {
            let c: Vec<String> = l.split('\t').map(str::to_string).collect();
            let id = u8::from_str_radix(c.first()?.trim_start_matches("0x"), 16).ok()?;
            Some((id, c))
        })
        .collect();
    let mut bad = Vec::new();
    for &(id, name, _) in MOVE_IDS {
        let size = HANDLED.iter().find(|&&(i, _)| i == id).map(|&(_, s)| s);
        match rows.get(&id) {
            Some(c)
                if c[1] == name
                    && c[6] == "handler"
                    && size.is_some_and(|s| c[2] == s.to_string()) => {}
            other => bad.push(format!("{id:#04x} {name}: {other:?} size {size:?}")),
        }
    }
    for &(id, _) in &HANDLED {
        if !MOVE_IDS.iter().any(|&(i, _, _)| i == id) {
            bad.push(format!("{id:#04x}: handled, not in MOVE_IDS"));
        }
    }
    bad
}

// Covers: specs/sim/intents-events.md §4 r1
#[test]
fn move_ids_match_client_tsv_and_the_module() {
    assert_eq!(ids_vs_tsv(CLIENT_TSV), Vec::<String>::new());
    assert!(MOVE_IDS.windows(2).all(|w| w[0].0 < w[1].0));
    // `handlers::items::ITEM_IDS` names the same owner for each, and
    // keeps 0x4C (`cube.md` §10) unowned.
    for &(id, _, spec) in MOVE_IDS {
        assert_eq!(
            ITEM_IDS.iter().find(|&&(i, _)| i == id),
            Some(&(id, Some(spec)))
        );
    }
    assert_eq!(
        ITEM_IDS.iter().find(|&&(i, _)| i == 0x4C),
        Some(&(0x4C, None))
    );
}

/// M08: a renamed row, a row whose kind is no longer `handler` and a
/// changed size are each reported, and nothing else.
#[test]
fn move_ids_check_reports_perturbations() {
    let renamed = CLIENT_TSV.replace("\tUnstackItems\t", "\tUnstackItemsX\t");
    let bad = ids_vs_tsv(&renamed);
    assert_eq!(bad.len(), 1, "{bad:?}");
    assert!(bad[0].starts_with("0x22 UnstackItems"));
    let stubbed = CLIENT_TSV.replace("0x0054D520\thandler", "0x0054D520\tstub0");
    let bad = ids_vs_tsv(&stubbed);
    assert_eq!(bad.len(), 1, "{bad:?}");
    assert!(bad[0].starts_with("0x63 ItemToBeltShift"));
    let resized = CLIENT_TSV.replace("0x50\tDropGold\t9\t", "0x50\tDropGold\t10\t");
    let bad = ids_vs_tsv(&resized);
    assert_eq!(bad.len(), 1, "{bad:?}");
    assert!(bad[0].starts_with("0x50 DropGold"));
}

// ---- 0x16, 0x17: ground ----------------------------------------------------------------

/// 0x16 to the cursor (§7.1, §8.2): result 0; mode 4, the cursor item,
/// out of the room list. The same frame's tick sends the update pass:
/// 0x9C action 1 (row 1), then 0x47, 0x48. The clean-up (§6.1 rule 4)
/// leaves nothing for the next tick.
// Covers: specs/items/inventory-moves.md §6.1 r2, §6.1 r4, §7.1 r2, §11
#[test]
fn pick_item_to_the_cursor() {
    let mut t = setup();
    let k = t.ground_item(KEY, 12, 11);
    assert!(t.in_room(k));
    let (code, bytes) = t.frame(&pick(k, 1));
    assert_eq!(code, Done);
    assert_eq!(t.mode(k), 4);
    let u = t.unit(k);
    assert_eq!(t.inventory().cursor(), u);
    assert!(!t.in_room(k));
    assert_eq!(bytes, t.pass_pick(&[x9c(0x01, k)]));
    assert_eq!(t.data(k).cmd_flags, 0, "clean-up");
    let p = t.player;
    // The reset clears +0xC8 bit 0; bit 1 ("save pending") stays (IS1).
    assert_eq!(t.sim().events.sys.units.get(p).unwrap().flags2 & 3, 2);
    assert_eq!(t.idle(), NO_BYTES);
    let me = t.pguid();
    assert_eq!(t.rest.take_log(), [format!("pickup_sound {me} {k}")]);
}

/// `cube.md` §8 rule 3, `audio/triggers-2.md` §14 rule 2: a sound queued
/// on the player (`0x00553380`; here event 1 with the player as target)
/// leaves as S→C 0x2C (the `d2-proto` PlaySound layout) in the player's
/// unit update, after the item messages and 0x47 / 0x48; the clean-up
/// clears it, so the next tick sends nothing.
// Covers: specs/world/cube.md §8 l2 r3; specs/audio/triggers-2.md §14 r2; specs/sim/intents-events.md §3.5 r4
#[test]
fn player_sound_follows_the_item_messages() {
    let mut t = setup();
    let k = t.ground_item(KEY, 12, 11);
    let p = t.player;
    d2_sim::units::sound::queue_sound(&mut t.sim().game, p, 1, Some(p)).unwrap();
    let (code, bytes) = t.frame(&pick(k, 1));
    assert_eq!(code, Done);
    let me = t.pguid();
    let mut want = t.pass(&[x9c(0x01, k)]);
    let sound = d2_proto::server::PlaySound {
        type_: 0,
        guid: me,
        event: 1,
    };
    want.push(sound.encode().to_vec());
    assert_eq!(bytes, want);
    assert_eq!(t.sim().game.sounds.get(p), None);
    assert_eq!(t.idle(), NO_BYTES);
}

/// 0x16 auto pickup (§8.1 step 7): the first free page-0 position of
/// the 10 × 4 grid, (9, 3); 0x9C action 4. Refusals: distance > 50 → 1;
/// distance ≥ 5 → walk, 0; type > 5 → 2; the own player → 3.
// Covers: specs/items/inventory-moves.md §7.1 r1, §7.1 r2
#[test]
fn pick_item_auto_and_refusals() {
    let mut t = setup();
    let k = t.ground_item(KEY, 12, 11);
    t.rest.with(|r| r.distance = 51);
    assert_eq!(t.frame(&pick(k, 0)), (Refused, NO_BYTES));
    t.rest.with(|r| r.distance = 5);
    assert_eq!(t.frame(&pick(k, 0)), (Done, NO_BYTES));
    let me = t.pguid();
    assert_eq!(t.rest.take_log(), [format!("walk_to_item {me} {k} false")]);
    assert_eq!(t.frame(&msg(0x16, &[6, k, 0])), (Invalid, NO_BYTES));
    assert_eq!(t.frame(&msg(0x16, &[0, me, 0])), (Malformed, NO_BYTES));
    t.rest.with(|r| r.distance = 1);
    let (code, bytes) = t.frame(&pick(k, 0));
    assert_eq!(code, Done);
    assert_eq!(t.mode(k), 0);
    let d = t.data(k);
    assert_eq!((d.page, d.x, d.y), (0, 9, 3));
    assert_eq!(bytes, t.pass_pick(&[x9c(0x04, k)]));
}

/// 0x17 (§7.2, §9.1): the cursor item dropped at the free spot: mode 3,
/// in the room, at the spot, expiry frame + 15000. §9.1 runs no owner
/// refresh and no update list, so the tick sends nothing to the owner;
/// the ground message (§6.3) is not sent: the per-unit update that would
/// send it is not wired (see `update_pass`). An item
/// that is not the cursor item → 1.
// Covers: specs/items/inventory-moves.md §7.2 r1, §9.1
#[test]
fn drop_item_to_the_ground() {
    let mut t = setup();
    let k = t.cursor_item(KEY);
    let room = t.room;
    t.rest.with(|r| {
        r.room_at = true;
        r.spot = Some(Spot { room, x: 13, y: 12 });
    });
    t.rest.take_log();
    assert_eq!(t.frame(&msg(0x17, &[k + 77])), (Refused, NO_BYTES));
    let (code, bytes) = t.frame(&msg(0x17, &[k]));
    assert_eq!(code, Done);
    assert_eq!(t.mode(k), 3);
    assert!(t.in_room(k));
    let d = t.data(k);
    assert_eq!((d.x, d.y, d.page), (13, 12, 0xFF));
    assert_eq!(t.inventory().cursor(), None);
    let u = t.unit(k).unwrap();
    let frame = t.sim().game.frame;
    assert_eq!(t.inv().state.expiry[&u], frame - 1 + 15000);
    assert_eq!(bytes, NO_BYTES);
    assert_eq!(t.rest.take_log(), [format!("quest_item_dropped {k}")]);
}

/// 0x17 with the ground announcement on (the play host's
/// `announce_ground`): the dropped item is announced in the drop's own
/// frame with 0x9C action 2 (§6.3: unit flag 0x1000, set by §9.1 step 3,
/// read before the room clean-up clears it), then nothing more. An item
/// placed on the ground without a drop is announced with action 0.
/// Recorded 2026-10-09: `facts/items/a1-town-item-moves.tsv` n 57–58
/// (C→S 0x17, S→C `9C 02 …` one frame later in 1.14d's numbering).
// Covers: specs/items/inventory-moves.md §6.3, §9.1
#[test]
fn a_drop_is_announced_with_action_2() {
    let mut t = setup();
    let k = t.cursor_item(KEY);
    let room = t.room;
    t.rest.with(|r| {
        r.room_at = true;
        r.spot = Some(Spot { room, x: 13, y: 12 });
    });
    t.sim().announce_ground = true;
    let (code, bytes) = t.frame(&msg(0x17, &[k]));
    assert_eq!(code, Done);
    assert_eq!(t.mode(k), 3);
    assert_eq!(bytes, [x9c(0x02, k)]);
    assert_eq!(t.idle(), NO_BYTES);
    let g = t.ground_item(KEY, 14, 12);
    assert_eq!(t.idle(), [x9c(0x00, g)]);
}

// ---- 0x18, 0x19: grid ------------------------------------------------------------------

/// 0x19 (§7.4): a stored key to the cursor: mode 4, stored page 0 →
/// 0x9D action 5 with the player as owner (row 4). 0x18 (§7.3) back at
/// (0, 0) of page 0: mode 0 → 0x9C action 4 (row 3). Page 1 → 2; a cell
/// outside the cube page → 3; lifting with a cursor item → 2.
// Covers: specs/items/inventory-moves.md §7.3 r1, §7.4 r1
#[test]
fn lift_and_insert() {
    let mut t = setup();
    let k = t.picked(KEY);
    let (code, bytes) = t.frame(&msg(0x19, &[k]));
    assert_eq!(code, Done);
    assert_eq!(t.mode(k), 4);
    assert_eq!(t.data(k).stored_page, 0);
    let m = t.owned(0x05, k);
    assert_eq!(bytes, t.pass(&[m]));
    assert_eq!(t.frame(&msg(0x18, &[k, 0, 0, 1])), (Invalid, NO_BYTES));
    assert_eq!(t.frame(&msg(0x18, &[k, 3, 0, 3])), (Malformed, NO_BYTES));
    let (code, bytes) = t.frame(&msg(0x18, &[k, 0, 0, 0]));
    assert_eq!(code, Done);
    assert_eq!(t.mode(k), 0);
    let d = t.data(k);
    assert_eq!((d.page, d.x, d.y), (0, 0, 0));
    assert_eq!(bytes, t.pass(&[x9c(0x04, k)]));
    let _c = t.cursor_item(KEY);
    // A cursor item: "can't do that" (S→C 0x5A, §7.4 step 2), 2.
    let (code, bytes) = t.frame(&msg(0x19, &[k]));
    assert_eq!(code, Invalid);
    assert_eq!(bytes, [d2_sim::items::moves::layouts::cant_do_that()]);
    assert_eq!(t.mode(k), 0);
}

/// 0x1F (§7.10): the cursor key C and the stored key T change places: T
/// to the cursor (stored page 0), C stored at the message's position
/// (2, 1); both command flag 0x40000 → 0x9C action 0xD (row 10, owner)
/// in update-list order.
// Covers: specs/items/inventory-moves.md §7.10 r1, §7.10 r2, §7.10 r3
#[test]
fn swap_cursor_buffer_item() {
    let mut t = setup();
    let tk = t.picked(KEY);
    let c = t.cursor_item(KEY);
    let (code, bytes) = t.frame(&msg(0x1F, &[c, tk, 2, 1]));
    assert_eq!(code, Done);
    let tu = t.unit(tk);
    assert_eq!(t.inventory().cursor(), tu);
    assert_eq!((t.mode(tk), t.mode(c)), (4, 0));
    let d = t.data(c);
    assert_eq!((d.page, d.x, d.y), (0, 2, 1));
    assert_eq!(bytes, t.pass(&[x9c(0x0D, tk), x9c(0x0D, c)]));
}

// ---- 0x1A–0x1E: body -------------------------------------------------------------------

/// 0x1A (§7.5, §4.6): a cap to the head: mode 1, body location 1 → 0x9D
/// action 6 (row 5). 0x1C (§7.7): off again → cursor, 0x9D action 8 (row
/// 7). Location 11 → 2; 0x1C with a cursor item does nothing.
// Covers: specs/items/inventory-moves.md §7.5, §7.7
#[test]
fn equip_and_remove_body_item() {
    let mut t = setup();
    let c = t.cursor_item(CAP);
    assert_eq!(t.frame(&body(0x1A, c, 11)), (Invalid, NO_BYTES));
    let (code, bytes) = t.frame(&body(0x1A, c, 1));
    assert_eq!(code, Done);
    assert_eq!(t.mode(c), 1);
    let u = t.unit(c);
    assert_eq!(t.inventory().body_item(1), u);
    let m = t.owned(0x06, c);
    assert_eq!(bytes, t.pass(&[m]));
    let (code, bytes) = t.frame(&loc16(0x1C, 1));
    assert_eq!(code, Done);
    assert_eq!(t.mode(c), 4);
    assert_eq!(t.inventory().cursor(), u);
    let m = t.owned(0x08, c);
    assert_eq!(bytes, t.pass(&[m]));
    assert_eq!(t.frame(&loc16(0x1C, 1)), (Done, NO_BYTES));
}

/// The equipment rules are on in play (`InvParts::new`): a cap whose
/// strength requirement (`inventory.md` §4.2) is over the character's
/// strength is refused and stays on the cursor; with enough strength it
/// is worn and its link holds, so the §5.7 inventory pass ends.
// Covers: specs/items/inventory.md §4.2, §5.7
#[test]
fn equip_over_strength_requirement_is_refused() {
    let mut t = setup();
    t.inv().tables.items[CAP].reqstr = 50;
    let c = t.cursor_item(CAP);
    let (code, bytes) = t.frame(&body(0x1A, c, 1));
    // §4.6: a failed requirement is result 0 (nothing moved, nothing sent).
    assert_eq!((code, bytes), (Done, NO_BYTES));
    let u = t.unit(c);
    assert_eq!(t.mode(c), 4);
    assert_eq!(t.inventory().body_item(1), None);
    assert_eq!(t.inventory().cursor(), u);
    let p = t.player;
    t.set_stat(p, 0, 50);
    assert_eq!(t.frame(&body(0x1A, c, 1)).0, Done);
    assert_eq!(t.mode(c), 1);
    assert_eq!(t.inventory().body_item(1), u);
}

/// 0x1B (§7.6): a two-handed sword onto the right hand over a shield in
/// the left: the shield leaves the body (mode 4, not linked; WV2), the
/// sword goes to location 4 → 0x9D action 7 (row 6). Location 3 → 3.
// Covers: specs/items/inventory-moves.md §7.6
#[test]
fn swap_two_handed_item() {
    let mut t = setup();
    let s = t.cursor_item(SHIELD);
    assert_eq!(t.frame(&body(0x1A, s, 5)).0, Done);
    let w = t.cursor_item(TWO_HANDER);
    // Two-handed is the items column (`wiring::inventory::queries`).
    let inv = t.sim().world.inventory.as_mut().unwrap();
    inv.tables.items[TWO_HANDER].twohanded = 1;
    assert_eq!(t.frame(&body(0x1B, w, 3)), (Malformed, NO_BYTES));
    let (code, bytes) = t.frame(&body(0x1B, w, 4));
    assert_eq!(code, Done);
    let wu = t.unit(w);
    assert_eq!(t.inventory().body_item(4), wu);
    assert_eq!(t.inventory().body_item(5), None);
    assert_eq!((t.mode(w), t.mode(s)), (1, 4));
    let m = t.owned(0x07, w);
    assert_eq!(bytes, t.pass(&[m]));
}

/// 0x1D (§7.8): a cap on the cursor over an equipped cap: E to the
/// cursor, N to the head; both command flag 0x20 → 0x9D action 9 (row 8)
/// in update-list order. An empty location → 1.
// Covers: specs/items/inventory-moves.md §7.8
#[test]
fn swap_cursor_with_body() {
    let mut t = setup();
    let e = t.cursor_item(CAP);
    assert_eq!(t.frame(&body(0x1A, e, 1)).0, Done);
    let n = t.cursor_item(CAP);
    assert_eq!(t.frame(&body(0x1D, n, 9)), (Refused, NO_BYTES));
    let (code, bytes) = t.frame(&body(0x1D, n, 1));
    assert_eq!(code, Done);
    let (eu, nu) = (t.unit(e), t.unit(n));
    assert_eq!(t.inventory().body_item(1), nu);
    assert_eq!(t.inventory().cursor(), eu);
    let m = [t.owned(0x09, e), t.owned(0x09, n)];
    assert_eq!(bytes, t.pass(&m));
}

/// 0x1E (§7.9): location 3 → 3; an empty location 4 → 1; a two-hander
/// onto a sword with the other hand empty: §4.3 gives 0, not 7 → 0 with
/// nothing moved.
// Covers: specs/items/inventory-moves.md §7.9
#[test]
fn swap_one_handed_with_two_handed() {
    let mut t = setup();
    let n = t.cursor_item(SWORD);
    assert_eq!(t.frame(&body(0x1E, n, 3)), (Malformed, NO_BYTES));
    assert_eq!(t.frame(&body(0x1E, n, 4)), (Refused, NO_BYTES));
    assert_eq!(t.frame(&body(0x1A, n, 4)).0, Done);
    assert_eq!(t.mode(n), 1);
    let n2 = t.cursor_item(TWO_HANDER);
    assert_eq!(t.frame(&body(0x1E, n2, 4)), (Done, NO_BYTES));
    assert_eq!((t.mode(n), t.mode(n2)), (1, 4));
}

// ---- 0x20–0x22: use, stack -------------------------------------------------------------

/// 0x20 (§7.11): a key is not `useable` → out 1 (step 1) → 3. A ground
/// item → 1.
// Covers: specs/items/inventory-moves.md §7.11
#[test]
fn use_grid_item() {
    let mut t = setup();
    let k = t.picked(KEY);
    let g = t.ground_item(KEY, 12, 12);
    assert_eq!(t.frame(&msg(0x20, &[g, 10, 10])), (Refused, NO_BYTES));
    assert_eq!(t.frame(&msg(0x20, &[k, 11, 10])), (Malformed, NO_BYTES));
}

/// 0x21 (§7.12): keys over the max stack (12): dst := 12, src := 3, both
/// announced (S→C 0x3E), dst 0x9C action 0xA (row 9). 0x22
/// (§7.13) on an owned item → 3 (X1).
// Covers: specs/items/inventory-moves.md §7.12, §7.13
#[test]
fn stack_and_unstack_items() {
    let mut t = setup();
    let dst = t.picked(KEY);
    let src = t.cursor_item(KEY);
    let (du, su) = (t.unit(dst).unwrap(), t.unit(src).unwrap());
    t.set_stat(du, stat::QUANTITY, 8);
    t.set_stat(su, stat::QUANTITY, 7);
    t.rest.take_log();
    let (code, bytes) = t.frame(&msg(0x21, &[src, dst]));
    assert_eq!(code, Done);
    assert_eq!(
        (t.stat(du, stat::QUANTITY), t.stat(su, stat::QUANTITY)),
        (12, 3)
    );
    // Both announced by S→C 0x3E (`units::messages::update_item_stat`,
    // the new base quantities) ahead of the update pass's 0x9C.
    assert_eq!(t.rest.take_log(), Vec::<String>::new());
    let mut want = vec![
        update_item_stat(dst, stat::QUANTITY, 12, 0),
        update_item_stat(src, stat::QUANTITY, 3, 0),
    ];
    want.extend(t.pass(&[x9c(0x0A, dst)]));
    assert_eq!(bytes, want);
    assert_eq!(t.frame(&msg(0x21, &[src, src])), (Malformed, NO_BYTES));
    assert_eq!(t.frame(&msg(0x22, &[dst])), (Malformed, NO_BYTES));
}

// ---- 0x23–0x26, 0x63: belt -------------------------------------------------------------

/// Auto pickup of a potion (§8.1 step 6) → slot 0, 0x9C action 0xE. 0x23
/// (§7.14) the cursor potion to slot 4 → 0x9C action 0xE. 0x24 (§7.15)
/// slot 0 back to the cursor → 0x9C action 0xF, and the compaction moves
/// slot 4 to 0 → 0x9D action 0x15 (row 20). 0x25 (§7.16) the cursor
/// potion and the belt potion change places → 0x9C action 0x10 twice.
// Covers: specs/items/inventory-moves.md §7.14, §7.15, §7.16
#[test]
fn belt_moves() {
    let mut t = setup();
    let a = t.ground_item(HP1, 12, 11);
    let (code, bytes) = t.frame(&pick(a, 0));
    assert_eq!(code, Done);
    assert_eq!(t.mode(a), 2);
    assert_eq!(bytes, t.pass_pick(&[x9c(0x0E, a)]));
    let b = t.cursor_item(HP1);
    let (code, bytes) = t.frame(&msg(0x23, &[b, 4]));
    assert_eq!(code, Done);
    assert_eq!((t.mode(b), t.data(b).x), (2, 4));
    assert_eq!(bytes, t.pass(&[x9c(0x0E, b)]));
    let (code, bytes) = t.frame(&msg(0x24, &[a]));
    assert_eq!(code, Done);
    assert_eq!((t.mode(a), t.data(b).x), (4, 0));
    let m = [x9c(0x0F, a), t.owned(0x15, b)];
    assert_eq!(bytes, t.pass(&m));
    let (code, bytes) = t.frame(&msg(0x25, &[a, b]));
    assert_eq!(code, Done);
    let bu = t.unit(b);
    assert_eq!(t.inventory().cursor(), bu);
    assert_eq!((t.mode(a), t.mode(b)), (2, 4));
    assert_eq!(bytes, t.pass(&[x9c(0x10, b), x9c(0x10, a)]));
}

/// 0x26 (§7.17): a belt potion used on the player. The dispatcher arms
/// it (item flag 0x4, `items/use.md` §1 step 5), so the targeting reset
/// after the use (`inventory.md` §5.3) clears it with S→C 0x3F before the
/// belt removal `0x00561E70` (0x9C action 0xF, bit-stream flag 0x20;
/// recorded 2026-10-09, `facts/items/a1-town-potions-low.tsv` n 10–11).
/// The host runs the potion itself (`items/use.md` §3.1); the rest's
/// `use_item` is not asked. The `healthpot` state goes on, so the pass
/// sends S→C 0xA8 state 100 with the recorded bytes `a8 00 <guid> 0a 64
/// ff 01` (n 12).
// Covers: specs/items/inventory-moves.md §7.17; specs/items/use.md §3.1
#[test]
fn use_belt_item() {
    let mut t = setup();
    let a = t.picked(HP1);
    t.rest.take_log();
    let (code, bytes) = t.frame(&msg(0x26, &[a, 0, 0]));
    assert_eq!(code, Done);
    let mut reset = vec![0x3F, 0xFF];
    reset.extend_from_slice(&a.to_le_bytes());
    reset.extend_from_slice(&[0xFF, 0xFF]);
    assert_eq!(bytes.len(), 3, "{bytes:02x?}");
    assert_eq!(bytes[0], reset);
    assert_eq!(&bytes[1][..2], &[0x9C, 0x0F]);
    // The stream (cut off here; its removal flag 0x20 is checked in
    // `d2_sim::wiring::inventory::tests::belt`) follows the header.
    assert_eq!(bytes[1], x9c(0x0F, a));
    let mut a8 = vec![0xA8, 0x00];
    a8.extend_from_slice(&t.pguid().to_le_bytes());
    a8.extend_from_slice(&[0x0A, 0x64, 0xFF, 0x01]);
    assert_eq!(bytes[2], a8);
    assert!(t.rest.take_log().is_empty());
}

/// 0x20 (§7.11) of a Town Portal scroll (`items/use.md` §4): in a town
/// the cast refuses and the scroll stays (no 0x9D, S→C 0x3F and 0x7C of
/// the failure reset); outside, used and consumed (0x9D with flag 0x20);
/// a tome is used and stays.
// Covers: specs/items/inventory-moves.md §7.11; specs/items/use.md §4
#[test]
fn use_town_portal_scroll_and_tome() {
    let mut t = setup();
    let s = t.picked(TSC);
    t.rest.with(|r| r.in_town = true);
    let (code, bytes) = t.frame(&msg(0x20, &[s, 0, 0]));
    assert_eq!(code, Done);
    assert!(!bytes.iter().any(|m| m[0] == 0x9D), "{bytes:?}");
    assert!(
        bytes.iter().any(|m| m[0] == 0x3F && m[1] == 0xFF),
        "{bytes:?}"
    );
    assert!(bytes.iter().any(|m| m[0] == 0x7C), "{bytes:?}");
    assert!(t.unit(s).is_some(), "the scroll stays");
    t.rest.with(|r| r.in_town = false);
    let (code, bytes) = t.frame(&msg(0x20, &[s, 0, 0]));
    assert_eq!(code, Done);
    assert!(bytes.iter().any(|m| m[0] == 0x9D), "{bytes:?}");
    // A tome is not consumed (§7.11 step 3, type 18); with no books row
    // its skill is −1, so its charge stays too.
    let b = t.picked(TBK);
    let (code, bytes) = t.frame(&msg(0x20, &[b, 0, 0]));
    assert_eq!(code, Done);
    assert!(!bytes.iter().any(|m| m[0] == 0x9D), "{bytes:?}");
}

/// 0x63 (§7.24): a stored potion to the first free belt slot. The two
/// messages go out **in the handler** (§6.4): 0x9D action 5 (page shown
/// as the stored page 0) and 0x9C action 0xE, then the tick's pass (owner
/// refresh, no update list): 0x47, 0x48. A cursor item → 2.
// Covers: specs/items/inventory-moves.md §6.4, §7.24 r1, §7.24 r2, §7.24 r3, §7.24 r4
#[test]
fn item_to_belt_shift_sends_now() {
    let mut t = setup();
    let a = t.cursor_item(HP1);
    assert_eq!(t.frame(&msg(0x18, &[a, 0, 0, 0])).0, Done);
    let (code, bytes) = t.frame(&msg(0x63, &[a]));
    assert_eq!(code, Done);
    assert_eq!((t.mode(a), t.data(a).x), (2, 0));
    let mut want = vec![t.owned(0x05, a), x9c(0x0E, a)];
    want.extend(t.relators());
    assert_eq!(bytes, want);
    let _c = t.cursor_item(KEY);
    assert_eq!(t.frame(&msg(0x63, &[a])), (Refused, NO_BYTES));
}

// ---- 0x27–0x29: item use, sockets, tomes -----------------------------------------------

/// 0x27 (§7.18): both items owned, but a cursor item exists → 0 (step
/// 2). A ground target → 1.
// Covers: specs/items/inventory-moves.md §7.18
#[test]
fn use_item_action() {
    let mut t = setup();
    let k = t.picked(KEY);
    let u = t.cursor_item(KEY);
    let g = t.ground_item(KEY, 12, 12);
    assert_eq!(t.frame(&msg(0x27, &[g, u])), (Refused, NO_BYTES));
    assert_eq!(t.frame(&msg(0x27, &[k, u])), (Done, NO_BYTES));
}

/// 0x28 (§7.19): the filler is not a socket filler (seam
/// `socket_filler`, default no) → nothing, 0; a filler not on the cursor
/// → the cursor check's result.
// Covers: specs/items/inventory-moves.md §7.19 r1, §7.19 r2
#[test]
fn socket_item() {
    let mut t = setup();
    let target = t.picked(KEY);
    let g = t.ground_item(KEY, 12, 12);
    assert_eq!(t.frame(&msg(0x28, &[g, target])), (Refused, NO_BYTES));
    let f = t.cursor_item(KEY);
    assert_eq!(t.frame(&msg(0x28, &[f, target])), (Done, NO_BYTES));
    assert_eq!((t.mode(f), t.mode(target)), (4, 0));
}

/// 0x29 (§7.20): a cursor scroll into a stored tome of the same spell:
/// tome quantity +1, the scroll freed (not consumed one by one: seam
/// default) and the cursor cleared with S→C 0x42 naming the player, then
/// the tome announced (0x3E); nothing else is sent (the recorded order:
/// `facts/items/a1-town-item-moves.tsv` frame 191). A second scroll of another spell → the original's fatal assert
/// (line 0x149C): result 3 and a recorded fault.
// Covers: specs/items/inventory-moves.md §7.20
#[test]
fn scroll_to_book_and_its_fatal() {
    let mut t = setup();
    let book = t.cursor_item(BOOK);
    assert_eq!(t.frame(&msg(0x18, &[book, 0, 0, 0])).0, Done);
    let bu = t.unit(book).unwrap();
    let q0 = t.stat(bu, stat::QUANTITY);
    let s = t.cursor_item(SCROLL);
    t.rest.take_log();
    assert_eq!(
        t.frame(&msg(0x29, &[s, book])),
        (
            Done,
            vec![
                vec![0x42, 0, 1, 0, 0, 0],
                update_item_stat(book, stat::QUANTITY, q0 + 1, 0)
            ]
        )
    );
    assert_eq!(t.stat(bu, stat::QUANTITY), q0 + 1);
    assert_eq!(t.unit(s), None, "freed");
    assert_eq!(t.inventory().cursor(), None);
    assert_eq!(t.rest.take_log(), Vec::<String>::new());

    let s2 = t.cursor_item(SCROLL);
    // The spell is item data +0x3E (suffix slot 0, `queries::spell_of`).
    let s2u = t.unit(s2).unwrap();
    t.sim().events.sys.hooks.items.get_mut(s2u).unwrap().suffix[0] = 7;
    assert_eq!(t.frame_raw(&msg(0x29, &[s2, book])), (Malformed, NO_BYTES));
    let faults = std::mem::take(&mut t.sim().world.action.faults);
    assert_eq!(
        faults,
        [WorldFault {
            client: 0,
            id: 0x29,
            error: WorldError::Move(MoveFatal::SpellMismatch),
        }]
    );
    assert_eq!(t.stat(bu, stat::QUANTITY), q0 + 1);
    t.no_errors();
}

// ---- 0x50, 0x61 ---------------------------------------------------------------------------

/// 0x50 (§7.22, §10.2): 1500 of 5000 gold: one `gld` pile made through
/// the real item creation on the host's economy, on the ground at the
/// spot, its gold 1500; the player's gold 3500; nothing sent. More than
/// the gold → 3; another unit's GUID → 3.
// Covers: specs/items/inventory-moves.md §7.22, §10.2
#[test]
fn drop_gold_makes_a_pile() {
    let mut t = setup();
    let p = t.player;
    t.set_stat(p, stat::GOLD, 5000);
    let room = t.room;
    t.rest.with(|r| {
        r.gold = true;
        r.spot = Some(Spot { room, x: 10, y: 10 });
    });
    let me = t.pguid();
    assert_eq!(t.frame(&msg(0x50, &[me, 5001])), (Malformed, NO_BYTES));
    assert_eq!(t.frame(&msg(0x50, &[me + 1, 10])), (Malformed, NO_BYTES));
    let before = t.sim().game.lists.units_of_type(UnitType::Item);
    t.rest.take_log();
    assert_eq!(t.frame(&msg(0x50, &[me, 1500])), (Done, NO_BYTES));
    let piles: Vec<UnitId> = t
        .sim()
        .game
        .lists
        .units_of_type(UnitType::Item)
        .into_iter()
        .filter(|u| !before.contains(u))
        .collect();
    assert_eq!(piles.len(), 1);
    let pile = piles[0];
    let g = t.sim().events.sys.units.get(pile).unwrap().guid;
    assert_eq!(
        t.sim().events.sys.units.get(pile).unwrap().class,
        GOLD as u32
    );
    assert_eq!(t.stat(pile, stat::GOLD), 1500);
    assert_eq!(t.mode(g), 3);
    assert!(t.in_room(g));
    assert_eq!(t.stat(p, stat::GOLD), 3500);
    assert_eq!(
        t.rest.take_log(),
        [
            format!("quest_item_dropped {g}"),
            format!("set_owner {g} {me}")
        ]
    );
}

/// 0x61 (§7.23): a classic game → 3; an expansion game without a
/// hireling (seam default) → 0, nothing changed.
// Covers: specs/items/inventory-moves.md §7.23 r1, §7.23 r2
#[test]
fn merc_item() {
    let mut t = setup_with(false);
    assert_eq!(t.frame(&loc16(0x61, 1)), (Malformed, NO_BYTES));
    let mut t = setup();
    let c = t.cursor_item(CAP);
    assert_eq!(t.frame(&loc16(0x61, 1)), (Done, NO_BYTES));
    assert_eq!(t.mode(c), 4);
}

/// 0x61 give (§7.23) with a hireling on the host's lists
/// (`WiredWorld::state.hirelings`, lent to the inventory wiring): the
/// cap on the cursor goes to the swap `0x0054CED0` (`hirelings.md` §11
/// rule 3): a duplicate (`0x0055A2A0`, a new GUID) in mode 1 at the
/// merc's body location 1 (`BodyLoc1` of `helm`), the merc's inventory
/// created, the player's cursor cleared.
// Covers: specs/items/inventory-moves.md §7.23 r2, §7.23 r3; specs/world/hirelings.md §11 r1, §11 r3; specs/world/hirelings-2.md §19
#[test]
fn merc_give_swaps_onto_the_hosts_hireling() {
    use d2_sim::world::hirelings::{HirelingTables, PetNode};
    let mut t = setup();
    let p = t.player;
    let room = t.room;
    t.rest.with(|r| r.no_used_skill = true);
    let req = AllocRequest {
        ty: UnitType::Monster,
        class: 0,
        room: Some(room),
        add: true,
        fixed_guid: None,
        mode: 1,
        allied: false,
    };
    let sim = t.sim();
    // Alive (`0x005541B0`): a player in mode 1.
    sim.events.sys.units.get_mut(p).unwrap().mode = 1;
    sim.events.sys.data.monsters = vec![d2_sim::units::hooks::MonsterInfo {
        mode_chart: false,
        enabled: true,
        aidel: [15; 3],
        moves: 0,
    }];
    let merc = sim
        .events
        .with(&mut sim.game, |g, v| v.allocate(g, &req, 12, 12))
        .unwrap();
    let mg = sim.events.sys.units.get(merc).unwrap().guid;
    sim.world.state.hireling_tables = Some(HirelingTables {
        rows: Default::default(),
        exp_ratios: Default::default(),
        max_level: 99,
        pet_flags: HirelingTables::WARP,
        pet_basemax: 1,
    });
    sim.world.state.hirelings.list_mut(p).nodes = vec![PetNode {
        guid: mg,
        ..PetNode::default()
    }];
    // The cap's requirements (`inventory.md` §4.2) on the merc.
    t.set_stat(merc, stat::LEVEL, 1);
    t.set_stat(merc, 0, 10);
    t.set_stat(merc, 2, 10);
    let c = t.cursor_item(CAP);
    let (code, _) = t.frame_raw(&loc16(0x61, 0));
    assert_eq!(code, Done);
    assert_eq!(t.inv().state.errors, vec![]);
    let inv = t.inv();
    let merc_inv = inv.state.inventories.get(&merc).expect("§11 rule 1");
    let copy = merc_inv.body_item(1).expect("§11 rule 3: equipped");
    assert!(inv.state.inventories[&p].cursor().is_none());
    let cg = t.sim().events.sys.units.get(copy).unwrap().guid;
    assert_ne!(cg, c);
    assert_eq!(t.mode(cg), 1);
    // The lists stay the host's.
    assert!(t.inv().state.hirelings.is_none());
}

// ---- host wiring ------------------------------------------------------------------------

/// A host without the inventory parts keeps every item-move id a stub
/// (result 0, recorded unhandled); with them, nothing is recorded.
// Covers: specs/sim/intents-events.md §4 r1
#[test]
fn without_inventory_parts_the_ids_stay_stubs() {
    let mut t = setup();
    let k = t.ground_item(KEY, 12, 11);
    let parts = t.sim().world.inventory.take();
    assert_eq!(t.frame(&pick(k, 1)), (Done, NO_BYTES));
    assert_eq!(t.mode(k), 3);
    assert_eq!(t.sim().unhandled, [(0, 0x16, 13)]);
    t.sim().world.inventory = parts;
    assert_eq!(t.frame(&msg(0x17, &[k])).0, Refused);
    assert_eq!(t.sim().unhandled.len(), 1);
}

// ---- early refusals ---------------------------------------------------------------------

impl T {
    /// Everything a refusal could touch, as text: the game, the units,
    /// stats and items, the inventory state, the player's fields, the
    /// stubs, and the seams' logs and outboxes.
    fn digest(&mut self) -> String {
        let p = self.player;
        let (log, sent) = self.rest.with(|r| (r.log.clone(), r.sent.clone()));
        let sim = self.sim();
        let s = &sim.events.sys;
        let w = &sim.world;
        format!(
            "{:?}|{:?}|{:?}|{:?}|{:?}|{:?}{:?}{:?}|{:?}{:?}{:?}{:?}|{log:?}{sent:?}",
            sim.game,
            s.units,
            s.stats,
            s.hooks.items,
            w.inventory.as_ref().map(|i| &i.state),
            sim.player_fields(p),
            sim.unhandled,
            sim.resyncs,
            s.hooks.x.sent,
            w.rest.sent,
            w.rest.log,
            w.action.faults,
        )
    }

    /// `m` through the dispatcher alone (no tick, so the digest compares
    /// the handler's effect only): result `code`, nothing changed,
    /// nothing queued for client 0.
    fn refused(&mut self, m: &[u8], code: ResultCode, what: &str) {
        let before = self.digest();
        let mut out = crate::buffers::ClientBuffers::new();
        out.add_client(0);
        let got =
            crate::dispatch::dispatch(self.sim(), &ProtoSizes, &mut out, 0, ALIVE.gate, m, m.len());
        assert_eq!(got, code, "{what}: {m:02X?}");
        assert_eq!(out.pop(0), None, "{what}: {m:02X?} sent a message");
        let after = self.digest();
        assert!(
            before == after,
            "{what}: {m:02X?} changed the host:\n{before}\n{after}"
        );
    }
}

/// The refusals `inventory-moves.md` §7 orders before any effect (the item,
/// cursor, stored, owned and location checks of §5.1, before the
/// targeting reset and the placement; `docs/HANDOFF.md` PK1) leave the
/// game, the inventories and the outgoing messages unchanged. The
/// refusals after a write are left out (§7.3 placement → 3 after the
/// page and the reset; §7.5 §4.6 → 3 after the reset; §7.10 after T
/// moved; §7.14, §7.19 after the reset; the item-move gate's 0, §5.4).
// Covers: specs/items/inventory.md §5.1
#[test]
fn early_refusals_change_nothing() {
    let mut t = setup();
    let k = t.picked(KEY);
    let g = t.ground_item(KEY, 12, 12);
    let c = t.cursor_item(CAP);
    let me = t.pguid();
    t.rest.take_log();
    // 0x16 (§7.1): the own player → 3; a missing item → 1; too far → 1.
    t.refused(&msg(0x16, &[0, me, 0]), Malformed, "§7.1 own player");
    t.refused(&pick(0xDEAD, 0), Refused, "§7.1 missing");
    t.rest.with(|r| r.distance = 51);
    t.refused(&pick(g, 0), Refused, "§7.1 distance");
    t.rest.with(|r| r.distance = 1);
    // 0x17 (§7.2): not the cursor item → 1.
    t.refused(&msg(0x17, &[k]), Refused, "§7.2 cursor");
    // 0x18 (§7.3): not the cursor item → 1; page 1 → 2; page 2 without
    // a trade → 3.
    t.refused(&msg(0x18, &[k, 0, 0, 0]), Refused, "§7.3 cursor");
    t.refused(&msg(0x18, &[c, 0, 0, 1]), Invalid, "§7.3 page 1");
    t.refused(&msg(0x18, &[c, 0, 0, 2]), Malformed, "§7.3 page 2");
    // 0x19 (§7.4): not stored → 1.
    t.refused(&msg(0x19, &[g]), Refused, "§7.4 stored");
    // 0x1A (§7.5): not the cursor item → 1; location 11 → 2.
    t.refused(&body(0x1A, k, 1), Refused, "§7.5 cursor");
    t.refused(&body(0x1A, c, 11), Invalid, "§7.5 location");
    // 0x1B (§7.6): location 11 → 2; location 3 → 3.
    t.refused(&body(0x1B, c, 11), Invalid, "§7.6 location");
    t.refused(&body(0x1B, c, 3), Malformed, "§7.6 not a hand");
    // 0x1C (§7.7): location 11 → 2.
    t.refused(&loc16(0x1C, 11), Invalid, "§7.7 location");
    // 0x1D (§7.8): location 11 → 2; an empty location → 1.
    t.refused(&body(0x1D, c, 11), Invalid, "§7.8 location");
    t.refused(&body(0x1D, c, 1), Refused, "§7.8 empty");
    // 0x1E (§7.9): location 3 → 3; an empty hand → 1.
    t.refused(&body(0x1E, c, 3), Malformed, "§7.9 not a hand");
    t.refused(&body(0x1E, c, 4), Refused, "§7.9 empty");
    // 0x20 (§7.11): a ground item → 1.
    t.refused(&msg(0x20, &[g, 10, 10]), Refused, "§7.11 stored");
    // 0x21 (§7.12): source = destination → 3; 0x22 (§7.13) → 3.
    t.refused(&msg(0x21, &[k, k]), Malformed, "§7.12 same item");
    t.refused(&msg(0x22, &[k]), Malformed, "§7.13");
    // 0x27 (§7.18): a ground item → 1; 0x28 (§7.19): not the cursor → 1.
    t.refused(&msg(0x27, &[g, k]), Refused, "§7.18 owned");
    t.refused(&msg(0x28, &[g, k]), Refused, "§7.19 cursor");
    // 0x50 (§7.22): more than the gold → 3; another unit → 3.
    t.refused(&msg(0x50, &[me, 1]), Malformed, "§7.22 amount");
    t.refused(&msg(0x50, &[me + 1, 1]), Malformed, "§7.22 unit");
    // 0x63 (§7.24): not stored → 1.
    t.refused(&msg(0x63, &[g]), Refused, "§7.24 stored");
    // 0x61 (§7.23): a classic game → 3.
    let mut t = setup_with(false);
    t.refused(&loc16(0x61, 1), Malformed, "§7.23 classic");
}

// ---- quests.md §9.1: a quest reward on the wired host's inventory model ---------------

/// A quest call giving `code` (`0x005466B0(game, player, code, 0, 2, 1)`).
struct Reward(UnitId, [u8; 4]);

impl crate::adapters::handlers::world::QuestCall for Reward {
    type Out = Option<UnitId>;
    fn call<W: d2_sim::world::quests::QuestWorld>(
        self,
        _: &mut QuestControl,
        w: &mut W,
    ) -> Option<UnitId> {
        w.reward_item(self.0, self.1, 0, 2, true)
    }
}

/// `WiredWorld::quests` lends the inventory model to the quest call: the
/// reward is created on the game's item store, placed in the player's
/// inventory (§2.4, page 0, send 1: the update list) and identified; the
/// next tick's update pass sends it (0x9C action 4, §6.1).
// Covers: specs/world/quests.md §9.1
#[test]
fn a_quest_reward_lands_in_the_hosts_inventory_model() {
    let mut t = setup();
    let p = t.player;
    let sim = &mut t.host.game;
    let item = WorldHost::quests(
        &mut sim.world,
        &mut sim.game,
        &mut sim.events,
        Reward(p, *b"key "),
    )
    .flatten()
    .expect("rewarded");
    assert!(sim.world.state.errors.is_empty());
    let it = sim.events.sys.hooks.items.get(item).expect("in the store");
    assert_ne!(it.flags & 0x10, 0);
    assert_eq!(sim.events.sys.units.get(item).unwrap().mode, 0);
    assert!(t.inv().state.holds(p, item));
    let g = t.sim().events.sys.units.get(item).unwrap().guid;
    let got = t.idle();
    assert!(
        got.iter()
            .any(|m| m[..2] == [0x9C, 4] && m[4..8] == g.to_le_bytes()),
        "{got:?}"
    );
}

// ---- the play host's rest (q-gold) ----------------------------------------------------

/// A gold pile of `amount` on the ground at the player's place.
fn gold_pile(t: &mut T, amount: i32) -> Guid {
    let p = t.player;
    let (x, y) = t.sim().events.hooks().path_position(p);
    let g = t.ground_item(GOLD, x, y);
    let u = t.unit(g).unwrap();
    t.set_stat(u, stat::GOLD, amount);
    g
}

/// 0x16 on a gold pile in reach, on the play host's rest: the pile's
/// amount is added to the player's stat 14 and the pile is freed
/// (`inventory-moves.md` §10.1).
// Covers: specs/items/inventory-moves.md §7.1 r2, §10.1
#[test]
fn preview_rest_picks_up_a_gold_pile() {
    let mut t = setup_preview();
    let p = t.player;
    t.set_stat(p, stat::GOLD, 100);
    let g = gold_pile(&mut t, 250);
    let (code, _) = t.frame(&pick(g, 0));
    assert_eq!(code, Done);
    assert_eq!(t.stat(p, stat::GOLD), 350);
    assert_eq!(t.unit(g), None, "the pile is freed");
}

/// Vector G2 on the play host's rest: level 1 (limit 10000), gold 9500,
/// a pile of 1000 picked up → gold 10000 and a new pile of the rest, 500,
/// on the ground (`inventory-moves.md` §10.1 `0x0055B030`, §10.2).
// Covers: specs/items/inventory-moves.md §10.1, §10.2
#[test]
fn preview_rest_leaves_the_gold_above_the_cap_as_a_pile() {
    let mut t = setup_preview();
    // As `preview_inv_parts`.
    t.inv().state.move_effects = true;
    let p = t.player;
    t.set_stat(p, stat::LEVEL, 1);
    t.set_stat(p, stat::GOLD, 9500);
    let g = gold_pile(&mut t, 1000);
    let (code, _) = t.frame(&pick(g, 0));
    assert_eq!(code, Done);
    assert_eq!(t.stat(p, stat::GOLD), 10000);
    assert_eq!(t.unit(g), None, "the picked pile is freed");
    let piles: Vec<_> = t
        .sim()
        .game
        .lists
        .units_of_type(UnitType::Item)
        .into_iter()
        .collect();
    assert_eq!(piles.len(), 1, "the rest stays as one new pile");
    assert_eq!(t.stat(piles[0], stat::GOLD), 500);
    assert!(t.sim().game.lists.unit(piles[0]).unwrap().room().is_some());
}

/// 0x21 over the max stack: each quantity change leaves as S→C 0x3E
/// (`inventory-moves.md` §7.12; layout `client/msg-stats-items.md` §5
/// r1), dst first, none through the rest's stat seam.
// Covers: specs/items/inventory-moves.md §7.12
#[test]
fn move_effects_announce_stack_quantities_with_0x3e() {
    use d2_sim::units::messages::update_item_stat;
    let mut t = setup();
    let dst = t.picked(KEY);
    let src = t.cursor_item(KEY);
    let (du, su) = (t.unit(dst).unwrap(), t.unit(src).unwrap());
    t.set_stat(du, stat::QUANTITY, 8);
    t.set_stat(su, stat::QUANTITY, 7);
    let (code, bytes) = t.frame(&msg(0x21, &[src, dst]));
    assert_eq!(code, Done);
    assert!(t
        .rest
        .take_log()
        .iter()
        .all(|l| !l.starts_with("send_item_stat")));
    let x3e: Vec<_> = bytes.into_iter().filter(|m| m[0] == 0x3E).collect();
    // The client's split reads size u8@1; the zero padding after it
    // arrives as 0x00 messages (`sim/intents-events.md` edge case 13).
    let sized = |m: Vec<u8>| m[..usize::from(m[1])].to_vec();
    assert_eq!(
        x3e,
        [
            sized(update_item_stat(dst, stat::QUANTITY, 12, 0)),
            sized(update_item_stat(src, stat::QUANTITY, 3, 0))
        ]
    );
}

/// 0x50 on the play host's rest: a pile is made (gold request, free
/// spot, staged room), the amount leaves stat 14 (`§7.22`, `§10.2`).
// Covers: specs/items/inventory-moves.md §7.22, §10.2
#[test]
fn preview_rest_drops_gold_into_a_pile() {
    let mut t = setup_preview();
    let p = t.player;
    t.set_stat(p, stat::GOLD, 5000);
    let me = t.pguid();
    let (code, _) = t.frame(&msg(0x50, &[me, 1500]));
    assert_eq!(code, Done);
    assert_eq!(t.stat(p, stat::GOLD), 3500);
    let piles: Vec<_> = t
        .sim()
        .game
        .lists
        .units_of_type(UnitType::Item)
        .into_iter()
        .collect();
    assert_eq!(piles.len(), 1);
    let pile = piles[0];
    assert_eq!(t.stat(pile, stat::GOLD), 1500);
    assert!(t.sim().game.lists.unit(pile).unwrap().room().is_some());
}

// ---- the corpse (q-corpse) -----------------------------------------------------------

/// The whole softcore path on the item-move fixture: a player with a cap
/// on the head, a key in the backpack and an item on the cursor dies;
/// at the corpse creation the cursor and body items move onto the corpse
/// and the grid stays (`vitals.md` §4.7 rule 1.7); after the respawn
/// 0x16 type 0 on the corpse takes everything back and the corpse is
/// freed (`inventory-moves.md` §7.1, §12).
// Covers: specs/combat/vitals.md §4.7; specs/items/inventory-moves.md §7.1, §12
#[test]
fn die_respawn_click_the_corpse_items_come_back() {
    use d2_sim::units::hooks::Sim as UnitSim;
    use d2_sim::units::modes::player_event1;

    let mut t = setup();
    let p = t.player;
    t.sim().events.sys.hooks.death.allocate_corpses = true;
    let cap = t.cursor_item(CAP);
    assert_eq!(t.frame(&body(0x1A, cap, 1)).0, Done);
    let key = t.picked(KEY);
    let cursor = t.cursor_item(SWORD);
    let (cap_u, key_u, sword_u) = (
        t.unit(cap).unwrap(),
        t.unit(key).unwrap(),
        t.unit(cursor).unwrap(),
    );

    t.set_stat(p, stat::GOLD, 1000);
    // The fixture's player record is in mode 0: on its feet (neutral).
    t.sim().events.sys.units.get_mut(p).unwrap().mode = 1;
    let room = t.room;
    t.rest.with(|r| {
        r.gold = true;
        r.spot = Some(Spot { room, x: 10, y: 10 });
    });

    // DT, then DD (ENDANIM by hand: the fixture has no animation data).
    {
        let sim = t.sim();
        sim.events.start_death(&mut sim.game, p);
        // The fixture has no AnimData row: the animation start's error.
        sim.events.sys.hooks.errors.clear();
    }
    t.idle();
    // The gold penalty (§4.6 rule 1): 1 % of 1000 is lost, the rest lies
    // in piles where the player died.
    assert_eq!(t.stat(p, stat::GOLD), 0);
    let items = t.sim().game.lists.units_of_type(UnitType::Item);
    let mut piles = 0;
    for u in items {
        if t.sim()
            .events
            .sys
            .units
            .get(u)
            .is_some_and(|r| r.class == GOLD as u32)
        {
            piles += t.stat(u, stat::GOLD);
        }
    }
    assert_eq!(piles, 990, "the gold left lies on the ground");
    {
        let sim = t.sim();
        let s = &mut sim.events.sys;
        let mut u = UnitSim {
            game: &mut sim.game,
            units: &mut s.units,
            stats: &mut s.stats,
            data: &s.data,
        };
        player_event1(&mut u, &mut s.hooks, p).unwrap();
        s.hooks.errors.clear();
    }
    t.idle();
    let corpses: Vec<UnitId> = t
        .sim()
        .game
        .lists
        .units_of_type(UnitType::Player)
        .into_iter()
        .filter(|&u| u != p)
        .collect();
    assert_eq!(corpses.len(), 1, "the corpse");
    let c = corpses[0];
    let cguid = t.sim().events.sys.units.get(c).unwrap().guid;
    // The cursor and body items are on the corpse; the grid stays.
    {
        let ci = &t.inv().state.inventories[&c];
        assert_eq!(ci.body_item(1), Some(cap_u), "the cap on the corpse's head");
        assert_eq!(ci.cursor(), None);
        assert!(ci.contains(sword_u), "the cursor item");
        assert!(!ci.contains(key_u));
    }
    assert_eq!(t.inventory().body_item(1), None);
    assert_eq!(t.inventory().cursor(), None);
    assert!(t.inventory().contains(key_u), "grid stays");
    assert!(!t.inventory().contains(sword_u));

    // The respawn (0x41 is the app test's): on its feet again.
    t.sim().events.sys.units.get_mut(p).unwrap().mode = 1;
    t.idle();

    // The click: 0x16 type 0 on the corpse.
    let (code, _) = t.frame(&msg(0x16, &[0, cguid, 0]));
    assert_eq!(code, Done);
    let inv = t.inventory();
    assert_eq!(inv.body_item(1), Some(cap_u), "the cap back on the head");
    assert!(inv.contains(sword_u), "the cursor item back");
    assert!(inv.contains(key_u));
    assert!(
        t.sim()
            .game
            .lists
            .find_unit(UnitType::Player, cguid)
            .is_none(),
        "the corpse is freed"
    );
}
