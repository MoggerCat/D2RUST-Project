// Spec: specs/audio/triggers-2.md §14; specs/world/objects.md §14; specs/sim/intents-events.md §7.3, §7.5
//! The sound-event slot on the action wiring: the object seam queues it
//! (`0x00553380`), the per-client object and monster updates flush it as
//! S→C 0x2C (`0x00571740`), the room clean-up clears it.

use std::sync::Arc;

use d2_data::tables::Objects;

use super::*;
use crate::tick::TickHooks;
use crate::units::lists::client_state::IN_GAME;
use crate::units::sound::play_sound_message;
use crate::units::ClientId;
use crate::world::objects::{ObjectTables, ObjectWorld};

/// One plain object row.
fn tables() -> Arc<ObjectTables> {
    Arc::new(ObjectTables {
        objects: vec![blank::<Objects>()],
        shrines: Vec::new(),
        levels: vec![blank(); 150],
        objgroup: Vec::new(),
        leveldefs: Vec::new(),
    })
}

fn guid(fx: &Fx, u: UnitId) -> u32 {
    fx.game.lists.unit(u).unwrap().guid
}

/// The fixture with an object and two players in room A, each player
/// with an in-game client in A; the object's unit flags cleared (no 0x0E).
fn setup() -> (Fx, UnitId, [(UnitId, ClientId); 2]) {
    let mut fx = Fx::new();
    fx.sim.create_objects(tables());
    let a = fx.a;
    let o = fx
        .sim
        .with(&mut fx.game, |g, v| v.create_object(g, a, 0, 20, 20, 0))
        .expect("allocated");
    fx.sim.sys.units.get_mut(o).unwrap().flags = 0;
    let mut players = [(o, ClientId(0)); 2];
    for (i, slot) in players.iter_mut().enumerate() {
        let p = fx.spawn(UnitType::Player, 0, a, 22 + i as i32, 20);
        let c = fx.game.lists.add_client(Some(p), Some(a), IN_GAME);
        *slot = (p, c);
    }
    (fx, o, players)
}

/// Runs the object seam's sound call.
fn sound(fx: &mut Fx, unit: UnitId, id: u8, to: Option<UnitId>, now: bool) {
    fx.sim
        .objects(&mut fx.game, |_, _, w| w.sound(unit, id, to, now))
        .expect("object state");
}

/// §14 rule 1: the seam stores event and target and queues the unit for
/// update; a second call overwrites both. Rule 2 / `objects.md` §14
/// rule 2: the object's per-client update sends one 8-byte 0x2C (type 2,
/// GUID, the last event) to each client while the target is none.
// Covers: specs/audio/triggers-2.md §14 r1, §14 r2; specs/world/objects.md §14 r2
#[test]
fn object_sound_is_flushed_per_client() {
    let (mut fx, o, [(p, c), (q, d)]) = setup();
    let a = fx.a;
    let _ = fx.game.lists.unqueue_update(o);
    sound(&mut fx, o, 22, Some(p), false);
    sound(&mut fx, o, 13, None, false);
    assert!(fx.game.lists.update_queue(a).contains(&o));
    assert!(fx.sim.hooks().x.sent.is_empty());
    fx.sim.send_unit_update(&mut fx.game, c, o);
    fx.sim.send_unit_update(&mut fx.game, d, o);
    let want = play_sound_message(UnitType::Object as u8, guid(&fx, o), 13);
    assert_eq!(&want[..2], &[0x2C, 2]);
    assert_eq!(
        fx.sim.hooks().x.sent,
        [(p, want.to_vec()), (q, want.to_vec())]
    );
    fx.assert_clean();
}

/// §14 rule 2: a target that is not the client's player: that client gets
/// nothing.
// Covers: specs/audio/triggers-2.md §14 r2
#[test]
fn targeted_sound_only_to_the_target_client() {
    let (mut fx, o, [(p, c), (_, d)]) = setup();
    sound(&mut fx, o, 19, Some(p), false);
    fx.sim.send_unit_update(&mut fx.game, d, o);
    assert!(fx.sim.hooks().x.sent.is_empty());
    fx.sim.send_unit_update(&mut fx.game, c, o);
    let want = play_sound_message(2, guid(&fx, o), 19);
    assert_eq!(fx.sim.hooks().x.sent, [(p, want.to_vec())]);
}

/// `intents-events.md` §7.5 step 3: the room clean-up clears flag 0x400 of
/// an object (no second send); a player's slot is left to the server's
/// player update pass.
// Covers: specs/sim/intents-events.md §7.5 r3
#[test]
fn room_cleanup_clears_the_slot() {
    let (mut fx, o, [(p, c), _]) = setup();
    sound(&mut fx, o, 13, None, false);
    sound(&mut fx, p, 22, None, false);
    fx.sim.unit_update(&mut fx.game, o);
    fx.sim.unit_update(&mut fx.game, p);
    assert_eq!(fx.game.sounds.get(o), None);
    assert!(fx.game.sounds.get(p).is_some());
    fx.sim.send_unit_update(&mut fx.game, c, o);
    assert!(fx.sim.hooks().x.sent.is_empty());
}

/// `objects.md` §8.1 rule 2: the key sound 11 is sent at once to the
/// operator's client (`0x00571740` called by the chest code).
// Covers: specs/world/objects.md §8.1 r2
#[test]
fn chest_key_sound_is_sent_at_once() {
    let (mut fx, _, [(p, _), _]) = setup();
    sound(&mut fx, p, 11, None, true);
    let want = play_sound_message(UnitType::Player as u8, guid(&fx, p), 11);
    assert_eq!(fx.sim.hooks().x.sent, [(p, want.to_vec())]);
    assert_eq!(fx.game.sounds.get(p).map(|s| s.event), Some(11));
}

/// `intents-events.md` §7.3 rule 2 step 6: the monster update sends the
/// 0x2C (type 1) to the client's player.
// Covers: specs/sim/intents-events.md §7.3 r2
#[test]
fn monster_update_flushes_the_sound() {
    let (mut fx, _, [(p, c), _]) = setup();
    fx.sim.hooks().enable_paths().expect("embedded tables");
    let a = fx.a;
    let m = fx.spawn(UnitType::Monster, 0, a, 25, 25);
    let r = fx.sim.sys.units.get_mut(m).unwrap();
    r.flags = 0;
    r.flags2 = 0;
    crate::units::sound::queue_sound(&mut fx.game, m, 16, None).unwrap();
    fx.sim.send_unit_update(&mut fx.game, c, m);
    let want = play_sound_message(UnitType::Monster as u8, guid(&fx, m), 16);
    assert_eq!(fx.sim.hooks().x.sent, [(p, want.to_vec())]);
}

/// A monster world holding only monster data (the step-10 read).
struct DataOnly(std::collections::BTreeMap<UnitId, crate::monsters::init::MonsterData>);

impl<X> crate::wiring::action::monsters::MonsterWorld<X> for DataOnly {
    fn type_init(
        &mut self,
        _: &mut crate::units::hooks::Sim<'_>,
        _: &mut ActionHooks<X>,
        _: UnitId,
    ) {
    }
    fn umods(
        &mut self,
        _: &mut crate::units::hooks::Sim<'_>,
        _: &mut ActionHooks<X>,
        _: UnitId,
        _: Option<UnitId>,
        _: u8,
    ) {
    }
    fn assign_umod(
        &mut self,
        _: &mut crate::units::hooks::Sim<'_>,
        _: &mut ActionHooks<X>,
        _: UnitId,
        _: u8,
    ) {
    }
    fn forget(&mut self, _: UnitId) {}
    fn monster(&self, unit: UnitId) -> Option<&crate::monsters::init::MonsterData> {
        self.0.get(&unit)
    }
    fn into_any(self: Box<Self>) -> Box<dyn std::any::Any> {
        self
    }
}

/// `intents-events.md` §7.3 rule 2 step 10: a monster with unit flag
/// 0x800 and monster data +0x5C bit 1 gets S→C 0x57 (`0x0053D880`: GUID,
/// type 1, name seed, umod bytes 0–2, type flag 4); without the +0x5C
/// bit, or without unit flag 0x800, nothing.
// Covers: specs/sim/intents-events.md §7.3 r2
#[test]
fn monster_update_sends_npc_enchants() {
    let (mut fx, _, [(p, c), _]) = setup();
    fx.sim.hooks().enable_paths().expect("embedded tables");
    let a = fx.a;
    let m = fx.spawn(UnitType::Monster, 0, a, 25, 25);
    let mut d = crate::monsters::init::MonsterData {
        name_seed: 0x1234,
        type_flags: 4 | 8,
        data_flag1: true,
        ..Default::default()
    };
    d.umods[..4].copy_from_slice(&[5, 17, 30, 9]);
    let world = DataOnly([(m, d.clone())].into_iter().collect());
    fx.sim.sys.hooks.monster_world = Some(Box::new(world));
    let set = |fx: &mut Fx, flags: u32| {
        let r = fx.sim.sys.units.get_mut(m).unwrap();
        r.flags = flags;
        r.flags2 = 0;
    };
    set(&mut fx, 0x800);
    fx.sim.send_unit_update(&mut fx.game, c, m);
    let g = guid(&fx, m).to_le_bytes();
    let want = [
        0x57, g[0], g[1], g[2], g[3], 1, 0x34, 0x12, 5, 17, 30, 0, 1, 0,
    ];
    assert_eq!(
        crate::units::messages::npc_enchants(guid(&fx, m), 0x1234, [5, 17, 30], true),
        want
    );
    assert_eq!(
        std::mem::take(&mut fx.sim.hooks().x.sent),
        [(p, want.to_vec())]
    );
    // No unit flag 0x800: nothing.
    set(&mut fx, 0);
    fx.sim.send_unit_update(&mut fx.game, c, m);
    assert_eq!(fx.sim.hooks().x.sent, []);
    // +0x5C bit 1 clear: nothing.
    d.data_flag1 = false;
    fx.sim.sys.hooks.monster_world = Some(Box::new(DataOnly([(m, d)].into_iter().collect())));
    set(&mut fx, 0x800);
    fx.sim.send_unit_update(&mut fx.game, c, m);
    assert_eq!(fx.sim.hooks().x.sent, []);
}

/// `intents-events.md` §7.9 rule 2: a unit's pending event records go to
/// each client's player in list order in the per-client update (monster
/// step 3; an object's update always, last) and the room clean-up frees
/// them (§7.5 step 2).
// Covers: specs/sim/intents-events.md §7.9 r2, §7.5 r2
#[test]
fn pending_event_records_are_sent_then_freed() {
    use crate::wiring::action::event_records::{preload, progressive, EventRecord};
    let (mut fx, o, [(p, c), _]) = setup();
    fx.sim.hooks().enable_paths().expect("embedded tables");
    let a = fx.a;
    let m = fx.spawn(UnitType::Monster, 0, a, 25, 25);
    let r = fx.sim.sys.units.get_mut(m).unwrap();
    r.flags = 0;
    r.flags2 = 0;
    let gm = guid(&fx, m);
    let a3 = EventRecord::Progressive {
        charges: 3,
        skill: 24,
        level: 5,
        unit: (1, gm),
        target: (0, 7),
        x: 0x1234,
        y: 0,
    };
    let recs = &mut fx.sim.sys.hooks.event_records;
    recs.push(m, a3);
    recs.push(m, EventRecord::Preload { class: 300 });
    recs.push(o, EventRecord::Preload { class: 9 });
    fx.sim.send_unit_update(&mut fx.game, c, m);
    assert_eq!(
        std::mem::take(&mut fx.sim.hooks().x.sent),
        [
            (
                p,
                progressive(3, 24, 5, (1, gm), (0, 7), 0x1234, 0).to_vec()
            ),
            (p, preload(300).to_vec()),
        ]
    );
    let r = fx.sim.sys.units.get_mut(o).unwrap();
    r.flags = 0;
    fx.sim.send_unit_update(&mut fx.game, c, o);
    assert_eq!(
        std::mem::take(&mut fx.sim.hooks().x.sent),
        [(p, preload(9).to_vec())]
    );
    fx.sim.unit_update(&mut fx.game, m);
    fx.sim.unit_update(&mut fx.game, o);
    assert_eq!(fx.sim.sys.hooks.event_records.of(m), []);
    fx.sim.send_unit_update(&mut fx.game, c, m);
    fx.sim.send_unit_update(&mut fx.game, c, o);
    assert_eq!(fx.sim.hooks().x.sent, []);
}
