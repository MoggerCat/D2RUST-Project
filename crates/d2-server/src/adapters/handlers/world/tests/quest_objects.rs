// Spec: specs/world/object-functions.tsv; specs/world/quests-act1-rest.md §1.1, §1.2, §2.2; specs/world/quests.md §9.5; specs/world/objects.md §3, §7.2
//! The object module's quest routes on the wired host: objects created
//! through the real allocation and init dispatch, the quest routes queued
//! by the action wiring and run on the host's quest control
//! (`d2_sim::wiring::economy::quest_objects`) through `HostQuests`
//! (object modes and timers on the real object state): a Cairn stone's
//! init after the tick that allocated it, the gibbet's operate right
//! after C→S 0x13, its event 7 when the timer fires, and a quest operate
//! no spec states handed back to `Pending::object_route`.

use std::sync::Arc;

use d2_data::tables::Objects;
use d2_sim::units::lifecycle::AllocRequest;
use d2_sim::units::{UnitId, UnitType};
use d2_sim::wiring::action::ObjectRoute;
use d2_sim::world::objects::{Dispatch, ObjectTables, Operate};

use super::trade_quests::Fx;
use super::waypoints::blank;
use super::*;
use crate::adapters::handlers::world::WorldHost;

const STONE: u32 = 17;
const GIBBET: u32 = 26;
const WIRT: u32 = 30;

/// objects.txt rows 0–30: a Cairn stone (17: init 6, operate 9), the
/// gibbet (26: operate 10, `FrameCnt1` 15 × 256), an object with quest
/// operate 33 (no quest spec states it).
fn tables() -> ObjectTables {
    let mut rows = vec![blank::<Objects>(); 31];
    rows[STONE as usize].initfn = 6;
    rows[STONE as usize].operatefn = 9;
    rows[GIBBET as usize].operatefn = 10;
    rows[GIBBET as usize].framecnt1 = 15 << 8;
    rows[WIRT as usize].operatefn = 33;
    ObjectTables {
        objects: rows,
        shrines: Vec::new(),
        levels: Vec::new(),
    }
}

fn fixture() -> Fx {
    let mut fx = Fx::new(|_| {});
    let ev = &mut fx.h.game.events;
    ev.create_objects(Arc::new(tables()));
    ev.route_quest_objects();
    ev.hooks().x.in_range = true;
    fx
}

impl Fx {
    fn object(&mut self, class: u32) -> UnitId {
        let req = AllocRequest {
            ty: UnitType::Object,
            class,
            room: None,
            add: true,
            fixed_guid: None,
            mode: 0,
            allied: false,
        };
        let s = &mut self.h.game;
        s.events
            .with(&mut s.game, |g, v| v.allocate(g, &req, 0, 0))
            .unwrap()
    }
    fn mode(&mut self, u: UnitId) -> u32 {
        self.h.game.events.sys.units.get(u).unwrap().mode
    }
    fn frames(&mut self, n: u32) {
        for _ in 0..n {
            self.h.clock.0 += 40;
            assert!(self.h.frame().unwrap().ticked);
        }
    }
}

/// C→S 0x13 for unit type 2 and `guid`.
fn operate(guid: u32) -> Vec<u8> {
    let mut m = vec![0x13, 2, 0, 0, 0];
    m.extend_from_slice(&guid.to_le_bytes());
    m
}

// Covers: specs/world/quests-act1-rest.md §2.2 r3; specs/world/objects.md §3
#[test]
fn a_cairn_stone_init_runs_on_the_quest_control_after_the_tick() {
    let mut fx = fixture();
    fx.world().quests.record_mut(4).unwrap().not_intro = false;
    let stone = fx.object(STONE);
    // Queued, not run, by the allocation.
    assert_eq!(fx.mode(stone), 0);
    fx.frames(1);
    // §2.2 step 3: the first stone 17 records its GUID and starts the
    // Tristram-portal timer; object mode := 2 on the real object.
    let g = fx.guid(stone);
    let x = &fx.world().quests.record(4).unwrap().extra.q4;
    assert_eq!(x.portal_stone, g);
    assert!(x.portal_timer);
    assert_eq!(fx.mode(stone), 2);
    assert!(fx.h.game.events.hooks().x.routes.is_empty());
    assert_eq!(fx.errors(), Vec::<String>::new());
}

// Covers: specs/world/quests-act1-rest.md §1.1 r4, §1.1 r5, §1.1 r6, §1.1 r8, §1.2 r3; specs/world/quests.md §9.5
#[test]
fn the_gibbet_operate_and_its_event_7_run_on_the_quest_control() {
    let mut fx = fixture();
    fx.world().quests.record_mut(4).unwrap().not_intro = true;
    let gibbet = fx.object(GIBBET);
    fx.frames(1);
    let g = fx.guid(gibbet);
    let (code, _) = send(&mut fx.h, &operate(g));
    assert_eq!(code, ResultCode::Done);
    // §1.1 step 4: mode 1 on the real object (the update pass sends it).
    assert_eq!(fx.mode(gibbet), 1);
    // Step 5: X +0x54 := 3; step 8: 4.13 and 4.1.
    assert_eq!(fx.world().quests.record(4).unwrap().extra.q4.gibbet_open, 3);
    let r = fx.record();
    let bit = |b: u8| u16::from_le_bytes([r[8], r[9]]) & (1 << b) != 0;
    assert!(bit(13) && bit(1));
    // Step 6: event 7 at the operate's frame + 17. The handler ran in
    // the drain, before the tick of `send`'s frame: at frame F = now − 1.
    let f = fx.h.game.game.frame - 1;
    while fx.h.game.game.frame < f + 16 {
        fx.frames(1);
        assert_eq!(fx.mode(gibbet), 1, "frame {}", fx.h.game.game.frame);
    }
    fx.frames(1);
    assert_eq!(fx.h.game.game.frame, f + 17);
    // §1.2 step 3 through `0x005449E0` class 26: mode 3.
    assert_eq!(fx.mode(gibbet), 3);
    assert!(fx.h.game.events.hooks().x.routes.is_empty());
}

// Covers: specs/world/objects.md §7.2 r4
#[test]
fn a_quest_operate_no_spec_states_is_handed_back() {
    let mut fx = fixture();
    let wirt = fx.object(WIRT);
    fx.frames(1);
    let g = fx.guid(wirt);
    let (code, _) = send(&mut fx.h, &operate(g));
    assert_eq!(code, ResultCode::Done);
    let p = fx.player;
    let want = ObjectRoute::Operate(Dispatch::Quest(Operate {
        object: wirt,
        operator: Some(p),
        class: WIRT as u16,
        operate_fn: 33,
    }));
    assert_eq!(fx.h.game.events.hooks().x.routes, vec![want]);
}

/// A quest call reading back the calls `HostQuests` answers.
struct Probe {
    player: UnitId,
    object: UnitId,
}

impl crate::adapters::handlers::world::QuestCall for Probe {
    type Out = (Option<(u8, u32)>, Option<(u8, u32)>, i32, i32);
    fn call<W: d2_sim::world::quests::QuestWorld>(
        self,
        _: &mut d2_sim::world::quests::QuestControl,
        w: &mut W,
    ) -> Self::Out {
        w.set_interact_unit(self.player, Some((2, 77)));
        let set = w.interact_unit(self.player);
        w.set_interact_unit(self.player, None);
        let reset = w.interact_unit(self.player);
        let len = w.object_anim_length(self.object);
        w.set_object_mode(self.object, 2);
        (set, reset, len, w.object_mode(self.object))
    }
}

// Covers: specs/world/npc.md §2; specs/world/objects.md §4
#[test]
fn host_quests_answer_from_the_owner_and_the_object_state() {
    let mut fx = fixture();
    let gibbet = fx.object(GIBBET);
    let probe = Probe {
        player: fx.player,
        object: gibbet,
    };
    let s = &mut fx.h.game;
    let out = WorldHost::quests(&mut s.world, &mut s.game, &mut s.events, probe).unwrap();
    // The interaction owner (the NPC rest), `objects.txt` FrameCnt1 as
    // stored, the mode set on the real object.
    assert_eq!(out, (Some((2, 77)), None, 15 << 8, 2));
    assert!(fx.world().rest.interact.is_empty());
    assert_eq!(fx.mode(gibbet), 2);
    // The rest's staged modes were not used.
    assert!(fx.world().rest.object_modes.is_empty());
}
