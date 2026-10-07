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
