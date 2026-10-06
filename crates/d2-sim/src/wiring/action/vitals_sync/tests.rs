// Spec: specs/combat/vitals.md §5.1–§5.3 (on the action fixture)
use super::*;
use crate::combat::vitals::sync::{life_mana_update2, SyncCache};
use crate::path::walk::messages::walk_verify;
use crate::wiring::action::tests::Fx;

/// A player of class 4 in room A at (10, 12) with life 40, max life 50,
/// mana 20, stamina 30 (×256), and an in-game client.
fn fx() -> (Fx, UnitId, ClientId) {
    let mut f = Fx::new();
    let a = f.a;
    let p = f.spawn(UnitType::Player, 4, a, 10, 12);
    f.stats(
        p,
        &[
            (stat::HITPOINTS, 40 << 8),
            (stat::MAXHP, 50 << 8),
            (stat::MANA, 20 << 8),
            (stat::STAMINA, 30 << 8),
        ],
    );
    let c = f
        .game
        .lists
        .add_client(Some(p), Some(a), client_state::IN_GAME);
    (f, p, c)
}

fn run_fx(f: &mut Fx, c: ClientId, queued: bool) -> Option<Vec<Vec<u8>>> {
    run(&mut f.sim, &mut f.game, c, (700, 800), queued)
}

// Covers: specs/combat/vitals.md §5.1 r1, §5.1 r3
#[test]
fn off_by_default_and_only_in_game() {
    let (mut f, p, c) = fx();
    assert_eq!(run_fx(&mut f, c, false), None, "sync off");
    f.sim.sys.hooks.enable_vitals_sync();
    f.game.lists.client_mut(c).unwrap().state = client_state::JOINING;
    assert_eq!(run_fx(&mut f, c, false), None, "state 3");
    f.game.lists.client_mut(c).unwrap().state = client_state::IN_GAME;
    f.game.lists.client_mut(c).unwrap().player = None;
    assert_eq!(run_fx(&mut f, c, false), None, "no player");
    f.game.lists.client_mut(c).unwrap().player = Some(p);
    assert!(run_fx(&mut f, c, false).is_some());
}

// Covers: specs/combat/vitals.md §5.1 r2, §5.2, §5.3 r2, §5.3 r3, §5.3 r6
#[test]
fn first_sync_sends_0x95_and_resets_the_counter() {
    let (mut f, p, c) = fx();
    f.sim.sys.hooks.enable_vitals_sync();
    f.game.lists.client_mut(c).unwrap().update_count = 7;
    // Cache all zero: Δlife 40 of 50 ≥ 10 %: 0x95 at the staged position.
    let got = run_fx(&mut f, c, false).unwrap();
    assert_eq!(
        got,
        [life_mana_update2(40, 20, 30, 700, 800, 0, 0).to_vec()]
    );
    assert_eq!(f.game.lists.client(c).unwrap().update_count, 0);
    let cache = f.sim.sys.hooks.sync.as_ref().unwrap().caches[&c];
    assert_eq!(
        cache,
        SyncCache {
            life: 40,
            mana: 20,
            stamina: 30,
            x: 700,
            y: 800,
            ..SyncCache::default()
        }
    );
    // Unchanged, not forced: nothing, the counter stays.
    f.game.lists.client_mut(c).unwrap().update_count = 9;
    assert_eq!(run_fx(&mut f, c, true), Some(vec![]));
    assert_eq!(f.game.lists.client(c).unwrap().update_count, 9);
    // Stamina only, forced at 10 with a queued buffer: 0x96.
    f.stats(p, &[(stat::STAMINA, 29 << 8)]);
    f.game.lists.client_mut(c).unwrap().update_count = 10;
    let got = run_fx(&mut f, c, true).unwrap();
    assert_eq!(got, [walk_verify(29, 700, 800, 0, 0).to_vec()]);
    assert_eq!(f.game.lists.client(c).unwrap().update_count, 0);
}

// Covers: specs/combat/vitals.md §5.3 r4, §5.3 r5
#[test]
fn gold_and_experience_follow() {
    let (mut f, p, c) = fx();
    f.sim.sys.hooks.enable_vitals_sync();
    run_fx(&mut f, c, false).unwrap();
    f.stats(p, &[(STAT_GOLD, 100), (STAT_EXPERIENCE, 300)]);
    f.game.lists.client_mut(c).unwrap().update_count = 20;
    let got = run_fx(&mut f, c, false).unwrap();
    assert_eq!(got, [vec![0x19, 100], vec![0x1B, 0x2C, 0x01]]);
}

// Covers: specs/combat/vitals.md §5.2
#[test]
fn life_prediction_reads_the_healthpot_list() {
    let (mut f, p, c) = fx();
    f.sim.sys.hooks.enable_vitals_sync();
    // A state 100 list: hpregen 128 per frame until frame + 50.
    let frame = f.game.frame;
    let s = &mut f.sim.sys;
    let l = s.stats.alloc(0x40, frame + 50, 0, 0);
    s.stats.set_state(l, STATE_HEALTHPOT);
    s.stats.set(&mut s.hooks, l, STAT_HPREGEN, 128, 0, None);
    s.stats.attach(&mut s.hooks, p, l, true);
    let now = current(&f.sim, &f.game, p, (0, 0));
    // q = (128 · 50 + 40 · 256) >> 8 = 65; 65 · 100 / 50 = 130 → low
    // byte 130 > 100 → 100.
    assert_eq!(now.lp, 100);
    let got = run_fx(&mut f, c, false).unwrap();
    assert_eq!(got[0][0], 0x18);
}
