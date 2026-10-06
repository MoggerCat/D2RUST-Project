// Spec: specs/world/npc.md §2–§4, §9; specs/world/vendors.md §3, §4, §7, §9; specs/world/quests.md §1.5; specs/client/bridge.md §3 (end to end)
//! End-to-end vendor path: the bridge (`d2_client::bridge`) on its local
//! link over the in-process `d2-server` host, whose game is `SimGame` on
//! the wired `d2-sim` (`wiring::action::ActionSim`) with the server's
//! [`TradeWorld`] as its world host: the NPC and vendor handlers run on
//! `wiring::interaction` (`Desk`, `VendorDesk`) over the action sim's own
//! units and stat lists, from synthetic tables (no game files).
//!
//! The run (`docs/handoff/e2e-vendor-host.md` lists where each step
//! stops and why):
//!
//! 1. C→S 0x13: Akara's talk starts (`npc.md` §2): S→C 0x27, 0x29, 0x28;
//! 2. C→S 0x2F: chat open (§3), the heal hook (§5);
//! 3. C→S 0x38 action 1: trade open (`vendors.md` §4) generates the store
//!    (§3) on the NPC-control seed, items created by the economy wiring
//!    on the game seed (`rng.md` §5.3);
//! 4. C→S 0x32 with too little gold: S→C 0x2A code 12 (§7.1 rule 4);
//! 5. C→S 0x32 with enough gold: **stops** at the item copy
//!    `0x0055A2A0` (`VendorRest::copy_item`, no items spec writes it):
//!    S→C 0x2A code 9 (§7.1 rule 9.2), nothing paid;
//! 6. C→S 0x33 of the player's buckler (re-sellable, not one of Akara's
//!    permanent codes): **stops** at the same copy into the NPC (§7.2
//!    rule 8): 0x2A code 9, nothing received;
//! 7. C→S 0x33 of the player's cap (a permanent code: no copy back,
//!    rule 8): the removal from the player is the inventory stub (rule
//!    9), the price is received (§9.1): 0x2A code 1, kind 3, the item's
//!    GUID, the new gold.
//!
//! Every S→C message the specs lay out is asserted byte for byte (0x2A
//! bytes 3–6 are not written by the original, `npc.md` §9; d2rs writes
//! 0). The run is repeated (same seed → identical transcript), and one
//! input is perturbed (M08): one gold more changes exactly the gold
//! fields of the 0x2A messages.

use std::sync::Arc;

use d2_client::bridge::dispatch::Dispatch;
use d2_client::bridge::link::{LinkError, Pumped, SendQueue, Sent, ServerLink};
use d2_client::bridge::local::{LocalLink, PendingSession};
use d2_client::bridge::Bridge;
use d2_data::bin::BinTable;
use d2_data::fixup::records::stat_ops;
use d2_data::tables::{Itemstatcost, Record};
use d2_proto::client::{BuyItem, EntityAction, InitEntityChat, InteractWithEntity, SellItem};
use d2_server::adapters::handlers::world::{ActionWorld, Outbox, TradeWorld};
use d2_server::adapters::{PlayerData, PlayerFields, ProtoSizes, SimGame, UnitFacts};
use d2_server::dispatch::Outcome;
use d2_server::host::{Handled, Host};
use d2_server::seams::{Clock, PlayerGate, Pos, ResultCode};
use d2_sim::combat::CombatTables;
use d2_sim::drlg::{DrlgData, Dungeon, LevelTypes, TileInfo, TileSource};
use d2_sim::game::Game;
use d2_sim::items::ItemRequest;
use d2_sim::rng::Seed;
use d2_sim::skills::SkillTables;
use d2_sim::stats::{StatData, StatTable};
use d2_sim::units::hooks::{MonsterInfo, UnitData};
use d2_sim::units::lifecycle::AllocRequest;
use d2_sim::units::lists::client_state;
use d2_sim::units::{UnitId, UnitType};
use d2_sim::wiring::action::{ActionHooks, ActionSim, ActionTables, DrlgWorld, Pending};
use d2_sim::wiring::economy::{GameFields, ItemSpawn};
use d2_sim::world::npc::{class, NpcControl};
use d2_sim::world::quests::{PlayerQuests, QuestControl, QuestTables};

mod e2e_support;
use e2e_support::*;

// ---- constants -----------------------------------------------------------------------

/// The game seed (`rng.md` §5.3).
const GAME_SEED: u32 = 1234;
/// Gold the player carries at the first buy: one less than the store
/// item's price is set later (step 4); this is the gold for steps 5–7.
const PLAYER_GOLD: i32 = 5000;
/// Stat ids (`itemstatcost`).
const LEVEL: u16 = 12;
const GOLD: u16 = 14;
const ARMORCLASS: u16 = 31;
const MAXDURABILITY: u16 = 73;
/// Item flags (`vendors.md` §3.1 rule 5, §7.2 rule 7).
const IDENTIFIED: u32 = 0x10;
/// Item mode "stored" (`vendors.md` §7.2 rule 9).
const STORED: u32 = 0;
const LOCAL: u32 = d2_client::bridge::LOCAL_CLIENT;

// ---- seams without a provider ----------------------------------------------------------

/// The action wiring's seams: every answer is `Pending`'s default (no
/// action path runs here); sends are kept for the world host.
#[derive(Default)]
struct ActionRest {
    sent: Vec<(UnitId, Vec<u8>)>,
}

impl Pending for ActionRest {
    fn send(&mut self, player: UnitId, msg: &[u8]) {
        self.sent.push((player, msg.to_vec()));
    }
}

impl Outbox for ActionRest {
    fn take_sent(&mut self) -> Vec<(UnitId, Vec<u8>)> {
        std::mem::take(&mut self.sent)
    }
}

struct NoTiles;

impl TileSource for NoTiles {
    fn dt1(&self, _: &[u8]) -> Option<&[TileInfo]> {
        None
    }
}

struct NoLevelTypes;

impl LevelTypes for NoLevelTypes {}

// ---- tables -----------------------------------------------------------------------------

/// A synthetic itemstatcost: 359 stats, no ops, fixed up.
fn stat_data() -> Arc<StatData> {
    let size = Itemstatcost::SIZE;
    let mut records = vec![0u8; N_STATS * size];
    for (s, r) in records.chunks_mut(size).enumerate() {
        for o in [0x32, 0x48, 0x4A, 0x56, 0x58, 0x5A, 0x5C] {
            r[o..o + 2].copy_from_slice(&0xFFFFu16.to_le_bytes());
        }
        r[0..2].copy_from_slice(&(s as u16).to_le_bytes());
    }
    let mut t = BinTable {
        name: "itemstatcost".into(),
        source: "synthetic".into(),
        count: N_STATS,
        record_size: size,
        records,
    };
    stat_ops(&mut t);
    Arc::new(StatData {
        stats: StatTable::from_fixed(&t).expect("itemstatcost"),
        ..StatData::default()
    })
}

// ---- the game ---------------------------------------------------------------------------

type World = TradeWorld<Rest>;
type Sim = SimGame<ActionSim<ActionRest>, World>;

/// Manual host clock (ms), injected into the host (`tick.md` §8).
struct Ms(u32);

impl Clock for Ms {
    fn now_ms(&mut self) -> u32 {
        self.0
    }
}

type Link = LocalLink<Sim, ProtoSizes, PendingSession, Ms>;

/// The local link, recording every S→C chunk the bridge receives.
struct Tap {
    inner: Link,
    chunks: Vec<Vec<u8>>,
}

impl ServerLink for Tap {
    fn protocol_version(&self) -> u32 {
        self.inner.protocol_version()
    }
    fn send(&mut self, queue: SendQueue, msg: &[u8]) -> Result<Sent, LinkError> {
        self.inner.send(queue, msg)
    }
    fn pump(&mut self) -> Result<Pumped, LinkError> {
        self.inner.pump()
    }
    fn receive(&mut self) -> Vec<Vec<u8>> {
        let got = self.inner.receive();
        self.chunks.extend(got.iter().cloned());
        got
    }
}

const ALIVE: PlayerGate = PlayerGate {
    mode: 1,
    uninterruptable: false,
};

/// Game creation before the first host frame: the action sim (no DRLG
/// rooms: the vendor path reads none), the NPC control and the quests
/// on the game seed, Akara and the player allocated, the player's gold
/// and level, a buckler and a cap the player owns, the client joined.
struct Fx {
    bridge: Bridge<Tap>,
    player: UnitId,
    npc: UnitId,
    /// The player's buckler (re-sellable) and cap (a permanent code).
    buckler: UnitId,
    cap: UnitId,
}

impl Fx {
    fn new(game_seed: u32, gold: i32) -> Self {
        let tables = ActionTables {
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
        };
        let drlg = DrlgWorld {
            dungeon: Dungeon::default(),
            data: Arc::new(DrlgData::default()),
            tiles: Box::new(NoTiles),
            types: Box::new(NoLevelTypes),
        };
        let hooks = ActionHooks::new(
            Arc::new(tables),
            drlg,
            Seed::init_low(game_seed),
            ActionRest::default(),
        );
        let data = UnitData {
            monsters: vec![
                MonsterInfo {
                    enabled: true,
                    aidel: [15; 3],
                    moves: 0,
                };
                N_MONSTATS
            ],
            ..UnitData::default()
        };
        let mut events = ActionSim::new(stat_data(), data, hooks);
        let mut game = Game::new();
        game.lists.ensure_act(0).unwrap();

        // Game creation: the NPC control (its own seed from the game
        // seed, `rng.md` §5.2) and the quests.
        let mut seed = events.hooks().game_seed;
        let ctl = NpcControl::new(&monstats(), Vec::new(), false, 0, &mut seed).expect("npc");
        let quests = QuestControl::new(&QuestTables::load().unwrap(), &mut seed).unwrap();
        events.hooks().game_seed = seed;

        let mut alloc = |ty, class| {
            let req = AllocRequest {
                ty,
                class,
                room: None,
                add: true,
                fixed_guid: None,
                mode: 1,
                allied: ty == UnitType::Player,
            };
            events
                .with(&mut game, |g, v| v.allocate(g, &req, 0, 0))
                .expect("allocated")
        };
        let npc = alloc(UnitType::Monster, u32::from(class::AKARA));
        let player = alloc(UnitType::Player, 1);
        // Players are allocated in mode 0; neutral (`units.md` §2).
        events.sys.units.get_mut(player).unwrap().mode = 1;
        events.with(&mut game, |_, v| {
            v.set_base(player, LEVEL, 1);
            v.set_base(player, GOLD, gold);
        });

        let mut rest = Rest::default();
        rest.quests.insert(player, PlayerQuests::default());
        let mut world: World = TradeWorld::new(
            ActionWorld::default(),
            GameFields::new(Seed::init_low(game_seed), false),
            item_tables(),
            quests,
            ctl,
            vendor_tables(),
            rest,
            1000,
        );
        // Monster init embeds the NPC's interaction list (`npc.md` §2).
        world.state.add_npc(npc);
        // The player's buckler and cap, made by the economy wiring on
        // the game seed (stored, mode 0).
        let (buckler, cap) = world.with_economy(&mut game, &mut events, |econ, _| {
            let mut make = |record: usize| {
                let mut rq = ItemRequest {
                    item: record as i32,
                    ilvl: 1,
                    quality: 2,
                    format: 1,
                    ..ItemRequest::default()
                };
                let spawn = ItemSpawn {
                    room: None,
                    mode: STORED,
                    init_flags: 1,
                };
                econ.create_item(&mut rq, false, spawn).expect("item")
            };
            (make(BUC), make(CAP))
        });
        world.rest.inventory.extend([buckler, cap]);

        let mut s: Sim = SimGame::with_world(game, events, world);
        s.join(LOCAL, Some(player), None, client_state::IN_GAME)
            .unwrap();
        s.set_player(
            player,
            PlayerFields {
                gate: ALIVE,
                data: Some(PlayerData { last_accept: 0 }),
            },
        );
        let at = UnitFacts {
            act: 0,
            pos: Pos { x: 100, y: 100 },
            owner: None,
        };
        s.set_unit(player, at);
        s.set_unit(npc, at);

        let link = LocalLink::new(Host::new(
            s,
            ProtoSizes,
            PendingSession::default(),
            Ms(1000),
        ));
        let tap = Tap {
            inner: link,
            chunks: Vec::new(),
        };
        let bridge = Bridge::with_dispatch(tap, Dispatch::from_spec().unwrap()).unwrap();
        Fx {
            bridge,
            player,
            npc,
            buckler,
            cap,
        }
    }

    fn sim(&mut self) -> &mut Sim {
        &mut self.bridge.link_mut().inner.host_mut().game
    }

    fn sim_ref(&self) -> &Sim {
        &self.bridge.link().inner.host().game
    }

    fn guid(&self, u: UnitId) -> u32 {
        self.sim_ref().game.lists.unit(u).unwrap().guid
    }

    fn stat(&mut self, u: UnitId, s: u16) -> i32 {
        let sim = self.sim();
        sim.events.with(&mut sim.game, |_, v| v.stat(u, s))
    }

    fn base(&mut self, u: UnitId, s: u16) -> i32 {
        self.sim_ref().events.sys.stats.unit_base(u, s, 0)
    }

    fn set_gold(&mut self, gold: i32) {
        let p = self.player;
        let sim = self.sim();
        sim.events
            .with(&mut sim.game, |_, v| v.set_base(p, GOLD, gold));
    }

    /// Every error so far: action adapters, the unit dispatch, the
    /// interaction adapters, the world handlers' faults.
    fn errors(&self) -> Vec<String> {
        let s = self.sim_ref();
        let mut e: Vec<String> = s
            .events
            .sys
            .hooks
            .errors
            .iter()
            .map(|e| format!("{e:?}"))
            .collect();
        e.extend(s.events.sys.errors.iter().map(|e| format!("{e:?}")));
        e.extend(s.world.state.errors.iter().map(|e| format!("{e:?}")));
        e.extend(s.world.action.faults.iter().map(|f| format!("{f:?}")));
        e
    }

    /// Sends `msgs` through the bridge, advances the host clock 40 ms
    /// (one tick) and runs one bridge frame: the drained (C→S id,
    /// result) pairs and the S→C chunks the client received.
    fn step(&mut self, msgs: &[Vec<u8>]) -> Frame {
        for m in msgs {
            assert_eq!(self.bridge.send_bytes(m).unwrap(), Sent::Queued);
        }
        self.bridge.link_mut().inner.host_mut().clock.0 += 40;
        let report = self.bridge.frame().unwrap();
        let host = self.bridge.link().inner.last_frame();
        let codes = host
            .messages
            .iter()
            .map(|h| match h.handled {
                Handled::Game(Outcome::Dispatched(c)) => (h.id, Some(c)),
                _ => (h.id, None),
            })
            .collect();
        let received = std::mem::take(&mut self.bridge.link_mut().chunks);
        assert_eq!(received.len(), report.messages, "one chunk per S→C message");
        Frame {
            sent: msgs.to_vec(),
            codes,
            received,
        }
    }
}

/// One bridge frame.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Frame {
    sent: Vec<Vec<u8>>,
    codes: Vec<(u8, Option<ResultCode>)>,
    received: Vec<Vec<u8>>,
}

fn bytes<M: d2_proto::FixedMessage>(m: &M) -> Vec<u8> {
    d2_client::bridge::intent::encode(m)
}

// ---- the run ----------------------------------------------------------------------------

/// A store item: (GUID, record, quality, item seed, AC, page).
type Row = (u32, usize, u8, Seed, i32, u8);

/// Everything a run leaves behind, compared between runs.
#[derive(Debug, PartialEq, Eq)]
struct Transcript {
    frames: Vec<Frame>,
    game_seed: Seed,
    npc_seed: Seed,
    store: Vec<Row>,
    gold: i32,
    rest_log: Vec<String>,
    unhandled: Vec<(u32, u8, usize)>,
    errors: Vec<String>,
}

fn run() -> Transcript {
    run_with(GAME_SEED, PLAYER_GOLD)
}

fn run_with(game_seed: u32, gold: i32) -> Transcript {
    let mut fx = Fx::new(game_seed, gold);
    let (player, npc) = (fx.player, fx.npc);
    let ng = fx.guid(npc);
    let mut frames = Vec::new();
    let done = Some(ResultCode::Done);

    // Frame 1: the tick driver starts its clock; no tick.
    assert!(!fx.bridge.frame().unwrap().ticked);

    // 1. C→S 0x13 (unit type 1, Akara): `npc.md` §2. Distance 3 (stub):
    // the AI parameter 0x28, the path cleared, the talk started: node
    // {player, state 0}, interact unit (1, GUID), S→C 0x27, 0x29, 0x28 in
    // that order (§2 start step 5).
    let talk = bytes(&InteractWithEntity { type_: 1, id: ng });
    let mut want = vec![0x13, 1, 0, 0, 0];
    want.extend_from_slice(&ng.to_le_bytes());
    assert_eq!(talk, want);
    let f = fx.step(&[talk]);
    assert_eq!(f.codes, [(0x13, done)]);
    let ids: Vec<u8> = f.received.iter().map(|m| m[0]).collect();
    assert_eq!(ids, [0x27, 0x29, 0x28]);
    // 0x27 (40 bytes): 0x27, u8 1, NPC GUID, then the 34-byte list of
    // `0x00661480` (`server-messages.tsv` 0x27 `partial`: the stub's
    // zeros).
    let mut npc_info = vec![0x27, 1];
    npc_info.extend_from_slice(&ng.to_le_bytes());
    npc_info.extend_from_slice(&[0; 34]);
    assert_eq!(f.received[0], npc_info);
    // 0x29 (97 bytes): 0x29 and the game record (`quests.md` §1.5).
    let mut game_quests = vec![0x29];
    game_quests.extend_from_slice(&fx.sim_ref().world.quests.game.0);
    assert_eq!(f.received[1], game_quests);
    assert_eq!(f.received[1].len(), 97);
    // 0x28 (103 bytes): 0x28, type 1, NPC GUID, 0, the player's record
    // of the current difficulty (§1.5).
    let mut quest_info = vec![0x28, 1];
    quest_info.extend_from_slice(&ng.to_le_bytes());
    quest_info.push(0);
    quest_info.extend_from_slice(&fx.sim_ref().world.rest.quests[&player].flags[0].0);
    assert_eq!(f.received[2], quest_info);
    assert_eq!(f.received[2].len(), 103);
    assert_eq!(fx.sim_ref().world.rest.interact[&player], (1, ng));
    frames.push(f);

    // 2. C→S 0x2F: chat open (§3): node state 0 → 1, the heal hook (§5;
    // nothing to heal on a fresh player). No message.
    let chat = bytes(&InitEntityChat { id: ng });
    let mut want = vec![0x2F, 0, 0, 0, 0];
    want.extend_from_slice(&ng.to_le_bytes());
    assert_eq!(chat, want);
    let f = fx.step(&[chat]);
    assert_eq!(f.codes, [(0x2F, done)]);
    frames.push(f);

    // 3. C→S 0x38 action 1 (trade): `vendors.md` §4 → §3. The store is
    // generated on the NPC-control seed (the buckler entry's range draw
    // and quality draws), its items made by the economy wiring on the
    // game seed (two steps per item, `rng.md` §5.3); then the permanent
    // cap. Each item: identified, vendor unit flag, page 0 (§3.1 rules
    // 3, 5). 0x9C action 11 belongs to the unwritten item spec
    // (`add_trade_inventory` stub): no S→C message.
    let seed_before = fx.sim_ref().events.sys.hooks.game_seed;
    let trade = bytes(&EntityAction {
        action: 1,
        npc: ng,
        item: 0,
    });
    assert_eq!(trade.len(), 13);
    let f = fx.step(&[trade]);
    assert_eq!(f.codes, [(0x38, done)]);
    assert!(f.received.is_empty());
    frames.push(f);
    let (store, rec_store) = {
        let w = &fx.sim_ref().world;
        let i = w.state.vendor_index(class::AKARA).unwrap();
        let rec = &w.state.vendors[i];
        assert!(rec.has_traded && rec.store_generated);
        assert_eq!(rec.last_npc, ng);
        (rec.store.clone(), rec.store.len())
    };
    assert!(rec_store >= 2, "≥ 1 buckler (Min 1) and the permanent cap");
    let mut seed = seed_before;
    for _ in 0..2 * rec_store {
        seed.step();
    }
    assert_eq!(fx.sim_ref().events.sys.hooks.game_seed, seed);
    let mut store_rows = Vec::new();
    for &item in &store {
        let guid = fx.guid(item);
        let ac = fx.base(item, ARMORCLASS);
        let s = fx.sim_ref();
        let it = s.world.items.get(item).unwrap();
        assert_ne!(it.flags & IDENTIFIED, 0);
        assert_eq!(it.inv_page, 0);
        let u = s.events.sys.units.get(item).unwrap();
        assert_ne!(u.flags2 & 4, 0, "vendor item (unit +0xC8 |= 4)");
        store_rows.push((guid, it.record, it.quality, it.item_seed, ac, it.inv_page));
    }
    let (bucs, caps): (Vec<&Row>, Vec<&Row>) = store_rows.iter().partition(|r| r.1 == BUC);
    assert!((1..=3).contains(&bucs.len()), "Min 1, Max 3");
    assert_eq!(caps.len(), 1);
    assert_eq!(caps[0].2, 2, "the permanent item is normal (§3)");
    let cap = *store.last().unwrap();
    assert_eq!(store_rows.last().unwrap().1, CAP, "permanent codes last");
    assert_eq!(fx.stat(cap, MAXDURABILITY), 12);

    // The cap's buy price by hand (`vendors.md` §9.2): S = cost 100;
    // type 50 (armor): S := cost·AC / max AC (rule 2); not magic, no
    // skills; Akara's sell mult 1024 (rule 9); qty 1; rp 0.
    let store_ac = fx.base(cap, ARMORCLASS);
    assert!((3..=5).contains(&store_ac));
    let price = 100 * store_ac / 5;
    let cg = fx.guid(cap);

    // 4. C→S 0x32 with one gold less than the price (§7.1 rule 4):
    // 0x2A code 12, GUID −1, result 0; nothing paid.
    fx.set_gold(price - 1);
    let buy = bytes(&BuyItem {
        npc: ng,
        item: cg,
        mode: 0,
        cost: 0,
    });
    let mut want = vec![0x32];
    for v in [ng, cg, 0, 0] {
        want.extend_from_slice(&v.to_le_bytes());
    }
    assert_eq!(buy, want);
    let f = fx.step(std::slice::from_ref(&buy));
    assert_eq!(f.codes, [(0x32, done)]);
    assert_eq!(f.received, [tx(0, 12, u32::MAX, price - 1)]);
    frames.push(f);
    assert_eq!(fx.stat(player, GOLD), price - 1);

    // 5. C→S 0x32 with `gold`: rules 1–8 pass (in the NPC inventory, the
    // price, gold, no cursor item, no tome, no stack), the purchase loop
    // copies the store item (rule 9.2). STOP: the copy `0x0055A2A0` is
    // written by no items spec; the stub's null runs the spec's refusal:
    // 0x2A code 9, GUID −1 (V10), result 1. Nothing paid.
    fx.set_gold(gold);
    // The same bytes again: past the client's 200 ms duplicate filter
    // (`bridge.md` §4 rule 5) first, five idle frames (nothing sent or
    // received).
    for _ in 0..5 {
        assert!(fx.step(&[]).received.is_empty());
    }
    let f = fx.step(&[buy]);
    assert_eq!(f.codes, [(0x32, Some(ResultCode::Refused))]);
    assert_eq!(f.received, [tx(0, 9, u32::MAX, gold)]);
    frames.push(f);
    assert_eq!(fx.stat(player, GOLD), gold);

    // 6. C→S 0x33 of the player's buckler (stored, mode 0): rules 1–7
    // pass (the player's per the staged inventory, mode 0, no flag, the
    // price, re-sellable). STOP at rule 8 (not a permanent code): the
    // copy into the NPC is the same unwritten `0x0055A2A0`: code 9, GUID
    // −1, result 3.
    let pg = fx.guid(fx.buckler);
    let sell = bytes(&SellItem {
        npc: ng,
        item: pg,
        tab: STORED as u16,
        cost: 0,
    });
    let mut want = vec![0x33];
    want.extend_from_slice(&ng.to_le_bytes());
    want.extend_from_slice(&pg.to_le_bytes());
    want.extend_from_slice(&[0; 8]);
    assert_eq!(sell, want);
    let f = fx.step(&[sell]);
    assert_eq!(f.codes, [(0x33, Some(ResultCode::Malformed))]);
    assert_eq!(f.received, [tx(0, 9, u32::MAX, gold)]);
    frames.push(f);
    assert_eq!(fx.stat(player, GOLD), gold);

    // 7. C→S 0x33 of the player's cap: re-sellable but one of Akara's
    // permanent codes, so no copy (rule 8: the permanent store item
    // stays); rule 9 takes it from the player (stored: `0x0055DF10`, the
    // inventory stub), rule 10 receives the price (§9.1; under the
    // staged carried-gold cap): 0x2A code 1, kind 3, the sold item's
    // GUID, the new gold. The price by hand (§9.2, t = 1): B = 100·AC/5
    // (rule 2), not ethereal, class 7 (rule 7: no /4), buy mult 512:
    // B·512/1024 (rule 9), qty 1, max buy 5000.
    let eg = fx.guid(fx.cap);
    let cap_ac = fx.base(fx.cap, ARMORCLASS);
    let sold = (100 * cap_ac / 5) * 512 / 1024;
    assert!(sold > 0);
    let sell = bytes(&SellItem {
        npc: ng,
        item: eg,
        tab: STORED as u16,
        cost: 0,
    });
    let f = fx.step(&[sell]);
    assert_eq!(f.codes, [(0x33, done)]);
    assert_eq!(f.received, [tx(3, 1, eg, gold + sold)]);
    frames.push(f);
    assert_eq!(fx.stat(player, GOLD), gold + sold);
    assert!(!fx.sim_ref().world.rest.inventory.contains(&fx.cap));

    let errors = fx.errors();
    assert!(errors.is_empty(), "{errors:?}");
    let gold_now = fx.stat(player, GOLD);
    let s = fx.sim_ref();
    // No id fell back to the stub: every message had a provider.
    assert!(s.unhandled.is_empty(), "{:?}", s.unhandled);
    Transcript {
        frames,
        game_seed: s.events.sys.hooks.game_seed,
        npc_seed: s.world.npc.seed,
        store: store_rows,
        gold: gold_now,
        rest_log: s.world.rest.log.clone(),
        unhandled: s.unhandled.clone(),
        errors,
    }
}

// Covers: specs/world/vendors.md §7.1 r4, §7.1 r9, §7.2 r8, §7.2 r10
#[test]
fn vendor_end_to_end() {
    let t = run();
    assert_eq!(t.frames.len(), 7);
    // Where each stub was reached, in order.
    let copies: Vec<&String> = t
        .rest_log
        .iter()
        .filter(|l| l.starts_with("copy"))
        .collect();
    assert_eq!(copies.len(), 2, "{:?}", t.rest_log);
    assert!(t.rest_log.iter().any(|l| l.starts_with("remove stored")));
}

/// Same seed → the same run: every C→S byte, result, S→C chunk, seed,
/// store item, gold and stub call, twice.
#[test]
fn same_seed_same_run() {
    assert_eq!(run(), run());
}

/// The comparison sees one input (M08): one gold more changes exactly
/// the gold fields (bytes 11–14) of the 0x2A messages of steps 5–7 and
/// the final gold; nothing else in the wire or the state.
#[test]
fn one_gold_more_changes_only_the_gold_fields() {
    let (a, b) = (run(), run_with(GAME_SEED, PLAYER_GOLD + 1));
    assert_eq!(b.gold, a.gold + 1);
    assert_eq!(a.store, b.store);
    assert_eq!((a.game_seed, a.npc_seed), (b.game_seed, b.npc_seed));
    assert_eq!(a.rest_log, b.rest_log);
    let mut diffs = Vec::new();
    for (i, (fa, fb)) in a.frames.iter().zip(&b.frames).enumerate() {
        assert_eq!(fa.sent, fb.sent);
        assert_eq!(fa.codes, fb.codes);
        assert_eq!(fa.received.len(), fb.received.len());
        for (j, (ma, mb)) in fa.received.iter().zip(&fb.received).enumerate() {
            for (k, (x, y)) in ma.iter().zip(mb).enumerate() {
                if x != y {
                    diffs.push((i, j, k));
                }
            }
        }
    }
    // Frames 4–6 (steps 5–7), the one 0x2A each, its gold's low byte.
    assert_eq!(diffs, [(4, 0, 11), (5, 0, 11), (6, 0, 11)]);
}

/// Another game seed gives other store items (unit and item seeds from
/// the game seed, the NPC-control seed from it too).
#[test]
fn other_seed_other_store() {
    let (a, b) = (run(), run_with(GAME_SEED + 1, PLAYER_GOLD));
    assert_ne!(a.game_seed, b.game_seed);
    assert_ne!(a.npc_seed, b.npc_seed);
    assert_ne!(a.store, b.store);
}
