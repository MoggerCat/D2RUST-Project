// Spec: specs/world/quests-act2.md §3.6, §4.9, §7.2; specs/world/quests.md §4.4
//! The Act II quest hooks the play host feeds once per tick
//! (`wired/quest_events.rs`, task `q-a2-quests`): Radament's and the
//! Summoner's AI calls and the cube's Horadric Staff hook, raised as the
//! pending seams queue them. PROVISIONAL (REC-136): the frame they run in.

use d2_sim::units::lifecycle::AllocRequest;
use d2_sim::units::{UnitId, UnitType};
use d2_sim::wiring::action::QuestEvent;

use super::trade_quests::Fx;

fn monster(f: &mut Fx, class: u32) -> UnitId {
    let req = AllocRequest {
        ty: UnitType::Monster,
        class,
        room: None,
        add: true,
        fixed_guid: None,
        mode: 1,
        allied: false,
    };
    let s = &mut f.h.game;
    s.events
        .with(&mut s.game, |g, v| v.allocate(g, &req, 0, 0))
        .unwrap()
}

fn queue(f: &mut Fx, e: QuestEvent) {
    f.h.game.events.hooks().x.quest_events.push(e);
}

fn frame(f: &mut Fx) {
    f.h.clock.0 += 40;
    assert!(f.h.frame().unwrap().ticked);
}

// Covers: specs/world/quests-act2.md §3.6
#[test]
fn radament_in_the_sewers_raises_the_lair_quest() {
    let mut f = Fx::new(|_| {});
    let rad = monster(&mut f, 229);
    f.world().quests.record_mut(8).unwrap().not_intro = true;
    f.world().quests.record_mut(8).unwrap().state = 2;
    // Outside Sewers Level 3 the hook does nothing.
    f.world().rest.levels.insert(rad, 48);
    queue(&mut f, QuestEvent::RadamentActivated { unit: rad });
    frame(&mut f);
    assert_eq!(f.world().quests.record(8).unwrap().state, 2);
    // In it: state := 3, status := 2.
    f.world().rest.levels.insert(rad, 49);
    queue(&mut f, QuestEvent::RadamentActivated { unit: rad });
    frame(&mut f);
    let r = f.world().quests.record(8).unwrap();
    assert_eq!((r.state, r.status), (3, 2));
}

// Covers: specs/world/quests-act2.md §7.2
#[test]
fn the_summoner_seen_starts_chain_12() {
    let mut f = Fx::new(|_| {});
    f.world().quests.record_mut(12).unwrap().not_intro = true;
    queue(&mut f, QuestEvent::SummonerActivated);
    frame(&mut f);
    let r = f.world().quests.record(12).unwrap();
    assert!(r.extra.a2.q5.seen);
    assert_eq!((r.state, r.status), (1, 2));
}

// Covers: specs/world/quests-act2.md §4.9
#[test]
fn an_assembled_staff_reaches_the_cube_quest() {
    let mut f = Fx::new(|_| {});
    let p = f.player;
    f.world().quests.record_mut(9).unwrap().not_intro = true;
    queue(&mut f, QuestEvent::StaffAssembled { player: p });
    frame(&mut f);
    let x = &f.world().quests.record(9).unwrap().extra.a2.q2;
    assert!(x.assembled);
}

// Covers: specs/world/quests-act2.md §4.9
#[test]
fn the_preview_cube_records_quest_item_hooks_for_the_host() {
    use crate::adapters::handlers::items::ItemPending;
    use crate::adapters::handlers::world::PreviewCubePending;
    let mut c = PreviewCubePending::default();
    c.quest_item_hook(UnitId(1), UnitId(1), *b"hst ");
    assert_eq!(c.take_quest_items(), vec![(UnitId(1), *b"hst ")]);
    assert!(c.take_quest_items().is_empty());
}
