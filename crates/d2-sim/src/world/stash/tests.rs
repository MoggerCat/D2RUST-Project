// Spec: specs/world/vendors-2.md §10.1, §10.2, §10.4
//! The stash buttons on a fake world: the §10.4 test vectors and the
//! dispatch rules.

use std::collections::BTreeMap;

use super::*;

const P: UnitId = UnitId(1);
const STASH: UnitId = UnitId(2);
const STASH_GUID: u32 = 77;

#[derive(Default)]
struct Fake {
    /// (active, type, GUID).
    interact: Option<(u8, u32)>,
    stats: BTreeMap<(UnitId, u16), i32>,
    objects: BTreeMap<u32, (UnitId, u32)>,
    town: Vec<UnitId>,
    sent: Vec<Vec<u8>>,
    sounds: Vec<(UnitId, u16, Option<UnitId>)>,
    drops: Vec<i32>,
    passes: u32,
    resets: u32,
}

impl Fake {
    /// P level 10 with gold g and goldbank s, at the stash in town.
    fn at_stash(g: i32, s: i32) -> Self {
        let mut f = Fake {
            interact: Some((INTERACT_OBJECT, STASH_GUID)),
            town: vec![P, STASH],
            ..Fake::default()
        };
        f.objects.insert(STASH_GUID, (STASH, STASH_CLASS));
        f.stats.insert((P, stat::LEVEL), 10);
        f.stats.insert((P, stat::GOLD), g);
        f.stats.insert((P, stat::GOLDBANK), s);
        f
    }
    fn gold(&self) -> (i32, i32) {
        (self.stat(P, stat::GOLD), self.stat(P, stat::GOLDBANK))
    }
    fn click(&mut self, button: u16, v: u32) -> ButtonRoute {
        click_button(self, P, button, v)
    }
}

impl StashWorld for Fake {
    fn interaction(&self, _: UnitId) -> Option<(u8, u32)> {
        self.interact
    }
    fn reset_interaction(&mut self, _: UnitId) {
        self.interact = None;
        self.resets += 1;
    }
    fn inventory_pass(&mut self, _: UnitId) {
        self.passes += 1;
    }
    fn send(&mut self, _: UnitId, msg: &[u8]) {
        self.sent.push(msg.to_vec());
    }
    fn object_by_guid(&self, guid: u32) -> Option<(UnitId, u32)> {
        self.objects.get(&guid).copied()
    }
    fn in_town(&self, unit: UnitId) -> bool {
        self.town.contains(&unit)
    }
    fn stat(&self, unit: UnitId, stat: u16) -> i32 {
        self.stats.get(&(unit, stat)).copied().unwrap_or(0)
    }
    fn set_base(&mut self, unit: UnitId, stat: u16, value: i32) {
        self.stats.insert((unit, stat), value);
    }
    fn add_base(&mut self, unit: UnitId, stat: u16, d: i32) {
        let v = self.stat(unit, stat).wrapping_add(d);
        self.stats.insert((unit, stat), v);
    }
    fn is_player(&self, unit: UnitId) -> bool {
        unit == P
    }
    fn drop_gold(&mut self, _: UnitId, amount: i32) {
        self.drops.push(amount);
    }
    fn sound(&mut self, unit: UnitId, event: u16, target: Option<UnitId>) {
        self.sounds.push((unit, event, target));
    }
}

/// §10.4 vectors 1–3: over the carried cap → sound 19 on P with target
/// P, nothing moves; in range → gold += v, goldbank −= v; v > goldbank →
/// nothing.
// Covers: specs/world/vendors-2.md §10.2 r2, §10.4 text
#[test]
fn withdraw_vectors() {
    let mut f = Fake::at_stash(60_000, 100_000);
    assert_eq!(f.click(BUTTON_WITHDRAW, 50_000), ButtonRoute::Done(0));
    assert_eq!(f.gold(), (60_000, 100_000));
    assert_eq!(f.sounds, [(P, SOUND_IMPOSSIBLE, Some(P))]);
    assert!(f.sent.is_empty());

    let mut f = Fake::at_stash(60_000, 100_000);
    assert_eq!(f.click(BUTTON_WITHDRAW, 40_000), ButtonRoute::Done(0));
    assert_eq!(f.gold(), (100_000, 60_000));
    assert!(f.sounds.is_empty() && f.sent.is_empty() && f.drops.is_empty());

    let mut f = Fake::at_stash(0, 100_000);
    assert_eq!(f.click(BUTTON_WITHDRAW, 100_001), ButtonRoute::Done(0));
    assert_eq!(f.gold(), (0, 100_000));
    assert!(f.sounds.is_empty());
    // v ≤ 0 (signed): nothing.
    assert_eq!(f.click(BUTTON_WITHDRAW, 0), ButtonRoute::Done(0));
    assert_eq!(f.gold(), (0, 100_000));
}

/// §10.4 vectors 4–6: a deposit over the cap fills the stash (partial);
/// a full stash takes nothing; v = 0x00010000 (p1 1, p2 0) moves 65,536.
// Covers: specs/world/vendors-2.md §10.2 r3, §10.4 text
#[test]
fn deposit_vectors() {
    let mut f = Fake::at_stash(30_000, 2_490_000);
    assert_eq!(f.click(BUTTON_DEPOSIT, 30_000), ButtonRoute::Done(0));
    assert_eq!(f.gold(), (20_000, 2_500_000));

    let mut f = Fake::at_stash(30_000, 2_500_000);
    assert_eq!(f.click(BUTTON_DEPOSIT, 1), ButtonRoute::Done(0));
    assert_eq!(f.gold(), (30_000, 2_500_000));

    let mut f = Fake::at_stash(70_000, 0);
    let v = button_value(1, 0);
    assert_eq!(v, 0x0001_0000);
    assert_eq!(f.click(BUTTON_DEPOSIT, v), ButtonRoute::Done(0));
    assert_eq!(f.gold(), (4_464, 65_536));
    // v > gold: nothing; no sound or message in any case.
    assert_eq!(f.click(BUTTON_DEPOSIT, 4_465), ButtonRoute::Done(0));
    assert_eq!(f.gold(), (4_464, 65_536));
    assert!(f.sounds.is_empty() && f.sent.is_empty());
}

/// §10.4 rule 2: p1 ≥ 0x8000 makes v negative; 0x13 and 0x14 do nothing.
// Covers: specs/world/vendors-2.md §10.4 r2
#[test]
fn negative_value_does_nothing() {
    let mut f = Fake::at_stash(60_000, 100_000);
    let v = button_value(0x8000, 5);
    assert_eq!(f.click(BUTTON_WITHDRAW, v), ButtonRoute::Done(0));
    assert_eq!(f.click(BUTTON_DEPOSIT, v), ButtonRoute::Done(0));
    assert_eq!(f.gold(), (60_000, 100_000));
    assert!(f.sounds.is_empty() && f.sent.is_empty());
}

/// §10.4 vectors 7–8: close resets the interaction (GUID −1, type 6,
/// inactive) and recounts, sends nothing; a second 0x12 finds no active
/// interaction and gets 0x77 0x0C, result 0.
// Covers: specs/world/vendors-2.md §10.2 r1, §10.1 r2
#[test]
fn close_twice() {
    let mut f = Fake::at_stash(0, 0);
    assert_eq!(
        f.click(BUTTON_STASH_CLOSE, 0xDEAD_BEEF),
        ButtonRoute::Done(0)
    );
    assert_eq!((f.interact, f.resets, f.passes), (None, 1, 1));
    assert!(f.sent.is_empty());
    assert_eq!(f.click(BUTTON_STASH_CLOSE, 0), ButtonRoute::Done(0));
    assert_eq!(f.sent, [vec![0x77, 0x0C]]);
    assert_eq!((f.resets, f.passes), (1, 1));
}

/// §10.1 rule 2 comes before the button test: any button with no
/// interaction gets 0x77 0x0C, result 0, the cube's too.
// Covers: specs/world/vendors-2.md §10.1 r2, §10 text
#[test]
fn no_interaction_any_button() {
    for b in [0, BUTTON_WITHDRAW, BUTTON_CUBE_TRANSMUTE, 0x30] {
        let mut f = Fake::at_stash(1_000, 1_000);
        f.interact = None;
        assert_eq!(f.click(b, 100), ButtonRoute::Done(0));
        assert_eq!(f.sent, [vec![0x77, 0x0C]]);
        assert_eq!(f.gold(), (1_000, 1_000));
    }
}

/// §10.1 rules 3–5: a stash button with interaction type ≠ 2 → result
/// 1, nothing sent (§10.4 last vector: type 4, the cube); 0x17 / 0x18 →
/// the cube; any other button with type ≠ 0 → 0x77 0x0D, result 3; type
/// 0 → the trade switch.
// Covers: specs/world/vendors-2.md §10.1 r3, §10.1 r4, §10.1 r5, §10.4 r3
#[test]
fn dispatch_routes() {
    let mut f = Fake::at_stash(60_000, 100_000);
    f.interact = Some((4, 9));
    for b in BUTTON_STASH_CLOSE..=BUTTON_DEPOSIT {
        assert_eq!(f.click(b, 40_000), ButtonRoute::Done(1));
    }
    assert!(f.sent.is_empty());
    assert_eq!(f.gold(), (60_000, 100_000));
    assert_eq!((f.resets, f.passes), (0, 0));
    assert_eq!(f.click(BUTTON_CUBE_CLOSE, 0), ButtonRoute::Cube);
    assert_eq!(f.click(BUTTON_CUBE_TRANSMUTE, 0), ButtonRoute::Cube);
    for b in [0, 1, 9, 0x11, 0x15, 0x16, 0x19, 0xFFFF] {
        f.sent.clear();
        assert_eq!(f.click(b, 0), ButtonRoute::Done(3));
        assert_eq!(f.sent, [vec![0x77, 0x0D]]);
    }
    // The stash interaction (type 2) is not type 0 either.
    let mut f = Fake::at_stash(0, 0);
    assert_eq!(f.click(2, 0), ButtonRoute::Done(3));
    f.interact = Some((INTERACT_PLAYER, 5));
    f.sent.clear();
    for b in [2, 8, 0x16] {
        assert_eq!(f.click(b, 0), ButtonRoute::Trade);
    }
    assert!(f.sent.is_empty());
}

/// §10.2 common checks: no stash object, another class, P or the stash
/// out of town → nothing at all; §10.4 rule 1: a 0x12 then leaves the
/// interaction set.
// Covers: specs/world/vendors-2.md §10.2 text, §10.4 r1
#[test]
fn common_checks() {
    type Break = fn(&mut Fake);
    let breaks: [Break; 4] = [
        |f| {
            f.objects.clear();
        },
        |f| {
            f.objects.insert(STASH_GUID, (STASH, 0x10A));
        },
        |f| f.town.retain(|&u| u != P),
        |f| f.town.retain(|&u| u != STASH),
    ];
    for brk in breaks {
        let mut f = Fake::at_stash(60_000, 100_000);
        brk(&mut f);
        for b in BUTTON_STASH_CLOSE..=BUTTON_DEPOSIT {
            assert_eq!(f.click(b, 40_000), ButtonRoute::Done(0));
        }
        assert_eq!(f.gold(), (60_000, 100_000));
        assert_eq!(f.interact, Some((INTERACT_OBJECT, STASH_GUID)));
        assert_eq!((f.resets, f.passes), (0, 0));
        assert!(f.sent.is_empty() && f.sounds.is_empty());
        // §10.4 rule 1: another button then gets 0x77 0x0D, result 3.
        assert_eq!(f.click(0x05, 0), ButtonRoute::Done(3));
        assert_eq!(f.sent, [vec![0x77, 0x0D]]);
    }
}

/// §10.2 rule 4: the clamped add sets 0 (not the cap) below 0 and, for a
/// player, over the carried cap (stat 14) or the stash cap (stat 15);
/// else it adds; a non-player is only clamped at 0.
// Covers: specs/world/vendors-2.md §10.2 r4
#[test]
fn clamped_add_rules() {
    let mut f = Fake::at_stash(500, 2_499_000);
    clamped_add(&mut f, P, stat::GOLD, -501);
    assert_eq!(f.stat(P, stat::GOLD), 0);
    f.set_base(P, stat::GOLD, 99_000);
    clamped_add(&mut f, P, stat::GOLD, 1_001);
    assert_eq!(f.stat(P, stat::GOLD), 0);
    f.set_base(P, stat::GOLD, 99_000);
    clamped_add(&mut f, P, stat::GOLD, 1_000);
    assert_eq!(f.stat(P, stat::GOLD), 100_000);
    clamped_add(&mut f, P, stat::GOLDBANK, 1_001);
    assert_eq!(f.stat(P, stat::GOLDBANK), 0);
    clamped_add(&mut f, P, stat::GOLDBANK, STASH_CAP);
    assert_eq!(f.stat(P, stat::GOLDBANK), STASH_CAP);
    // Another unit: no cap test.
    let q = UnitId(9);
    clamped_add(&mut f, q, stat::GOLDBANK, STASH_CAP + 1);
    assert_eq!(f.stat(q, stat::GOLDBANK), STASH_CAP + 1);
    clamped_add(&mut f, q, stat::GOLD, -(STASH_CAP + 2));
    assert_eq!(f.stat(q, stat::GOLD), 0);
}
