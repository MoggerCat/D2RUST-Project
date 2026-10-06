//! Tests written from the item specs to kill mutants that survived
//! `cargo mutants` on `crates/d2-sim/src/items/` (METHODS M08;
//! `docs/handoff/mutants-items-treasure.md`). Each test asserts what the
//! spec rule says at the boundary or bit the mutant changed.

use super::*;
use crate::items::affixes::{crafted, magic, rare};
use crate::items::create::{class_skill_mods, init_item_stats, socket_count, socket_roll};
use crate::items::tables::{RareRec, SkillRec};
use crate::items::{create_item, flag, q, req, stat, ItemRequest, PlayerInfo, RequestUnit};

/// `quality.md` §8.1: bit 4096 is a real bit; only an index above 4096
/// reads as dropped.
// Covers: specs/items/quality.md §8.1
#[test]
fn unique_bit_4096_is_a_bit() {
    let mut b = UniqueBits::default();
    assert!(!b.get(4096));
    assert!(b.get(4097));
    b.set(4096);
    assert!(b.get(4096));
    assert!(!b.get(4095));
}

/// Tables for the rare kind-order model: one magic suffix (group 2), one
/// magic prefix (group 1), one rare suffix and one rare prefix, all
/// fitting a helm.
fn rare_tables() -> (ItemTables, usize) {
    let mut t = tables();
    let i = push_item(&mut t, item_rec(ty::HELM, b"cap "));
    t.magic = vec![affix_row(ty::HELM, 2), affix_row(ty::HELM, 1)];
    t.n_suffix = 1;
    t.n_prefix = 1;
    let r = RareRec {
        itype: [ty::HELM as i16, 0, 0, 0, 0, 0, 0],
        ..Default::default()
    };
    t.rare = vec![r.clone(), r];
    t.n_rare_suffix = 1;
    (t, i)
}

/// `affixes.md` §7 on [`rare_tables`], step by step: rare names draw
/// roll(1) each, the count one step, then per affix the kind step (lo′
/// odd → suffix; only while both kinds are open) and §3 (coin step; with
/// the kind's slot empty one roll(2) step and the row, else its group is
/// taken: no candidate, 0). Returns the seed and the slot counts.
fn rare_model(mut s: Seed) -> (Seed, usize, usize) {
    s.roll(1);
    s.roll(1);
    let n = [3, 4, 4, 5, 5, 5, 6, 6][(s.step() & 7) as usize];
    let (mut np, mut ns, mut pdone, mut sdone, mut k) = (0, 0, false, false, 0);
    while k < n {
        if pdone && sdone {
            break;
        }
        let suffix = if pdone {
            true
        } else if sdone {
            false
        } else {
            s.step() % 2 == 1
        };
        s.step(); // coin
        let filled = if suffix { ns } else { np };
        if filled > 0 {
            if suffix {
                sdone = true;
            } else {
                pdone = true;
            }
            continue;
        }
        s.roll(2);
        if suffix {
            ns += 1;
        } else {
            np += 1;
        }
        k += 1;
    }
    (s, np, ns)
}

/// `affixes.md` §7 step 4.2: the kind step picks a suffix on an odd lo′,
/// a prefix on an even one.
// Covers: specs/items/affixes.md §7 r4
#[test]
fn rare_kind_step_parity() {
    let (t, i) = rare_tables();
    let rq = ItemRequest::default();
    let mut orders = std::collections::BTreeSet::new();
    for seed in 0..200 {
        let mut it = item(i, seed);
        it.quality = q::RARE;
        assert!(rare(&t, &mut it, &rq));
        let (s, np, ns) = rare_model(Seed::init_low(seed));
        assert_eq!(it.item_seed, s, "seed {seed}");
        assert_eq!((it.prefix[0] != 0, it.suffix[0] != 0), (np == 1, ns == 1));
        // Which kind came first (the first kind step).
        let mut f = Seed::init_low(seed);
        f.roll(1);
        f.roll(1);
        f.step();
        orders.insert(f.step() & 1);
    }
    assert_eq!(orders.len(), 2, "both first kinds exercised");
}

/// `affixes.md` §7 step 5: success needs P or S > 0; suffixes alone pass.
/// Step 4.3: the third suffix closes the suffix kind (no fourth slot).
// Covers: specs/items/affixes.md §7 r5
#[test]
fn rare_with_suffixes_only() {
    let mut t = tables();
    let i = push_item(&mut t, item_rec(ty::HELM, b"cap "));
    // Six suffixes in distinct groups, no prefix row.
    t.magic = (0..6).map(|g| affix_row(ty::HELM, g + 1)).collect();
    t.n_suffix = 6;
    t.n_prefix = 0;
    let r = RareRec {
        itype: [ty::HELM as i16, 0, 0, 0, 0, 0, 0],
        ..Default::default()
    };
    t.rare = vec![r.clone(), r];
    t.n_rare_suffix = 1;
    let rq = ItemRequest::default();
    let mut seen_full = false;
    for seed in 0..60 {
        let mut it = item(i, seed);
        it.quality = q::RARE;
        assert!(rare(&t, &mut it, &rq), "seed {seed}");
        assert_eq!(it.prefix, [0; 3]);
        assert_ne!(it.suffix[0], 0);
        seen_full |= it.suffix.iter().all(|&a| a != 0);
    }
    assert!(seen_full, "a count ≥ 4 fills all three suffix slots");
}

/// `affixes.md` §8 step 2: the crafted minimum from the request ilvl
/// (> 70 → 4, > 50 → 3, > 30 → 2, else 1) when lo′ mod 5 = 0.
// Covers: specs/items/affixes.md §8 r2
#[test]
fn crafted_minimum_by_ilvl() {
    let mut t = tables();
    let i = push_item(&mut t, item_rec(ty::HELM, b"cap "));
    t.magic = (0..12).map(|g| affix_row(ty::HELM, g + 1)).collect();
    t.n_suffix = 6;
    t.n_prefix = 6;
    let r = RareRec {
        itype: [ty::HELM as i16, 0, 0, 0, 0, 0, 0],
        ..Default::default()
    };
    t.rare = vec![r.clone(), r];
    t.n_rare_suffix = 1;
    // Two rare-name roll(1) steps, then the count step with lo′ mod 5 = 0.
    let seed = find_seed(|s| {
        s.step();
        s.step();
        s.step() % 5 == 0
    });
    for (ilvl, want) in [(30, 1), (31, 2), (50, 2), (51, 3), (70, 3), (71, 4)] {
        let rq = ItemRequest {
            ilvl,
            ..Default::default()
        };
        let mut it = item(i, seed);
        it.quality = q::CRAFTED;
        assert_eq!(crafted(&t, &mut it, &rq), Ok(true));
        let n = it
            .prefix
            .iter()
            .chain(&it.suffix)
            .filter(|&&a| a != 0)
            .count();
        assert_eq!(n, want, "ilvl {ilvl}");
    }
}

/// `affixes.md` §6 step 3: success clears only the identified flag.
// Covers: specs/items/affixes.md §6 r3
#[test]
fn magic_clears_identified_only() {
    let (t, i) = rare_tables();
    let mut it = item(i, 1);
    it.quality = q::MAGIC;
    it.flags = flag::IDENTIFIED | flag::INSTORE | flag::INIT;
    let rq = ItemRequest {
        prefix: [1, 0, 0],
        ..Default::default()
    };
    assert!(magic(&t, &mut it, &rq));
    assert_eq!(it.flags, flag::INSTORE | flag::INIT);
}

/// `generation.md` §4 r1 / `treasure.md` §8 r1: a quantity > 0 replaces
/// the gold base; 0 or less keeps the roll.
// Covers: specs/items/generation.md §4 r1
#[test]
fn gold_quantity_override() {
    let mut t = tables();
    let g = push_item(&mut t, item_rec(ty::GOLD, b"gld "));
    for (ovr, replaced) in [(77, true), (0, false), (-5, false)] {
        let mut it = item(g, 1);
        it.ilvl = 10;
        it.unit_seed = Seed::init_low(9);
        let mut rq = ItemRequest {
            ilvl: 10,
            quantity_override: ovr,
            ..Default::default()
        };
        init_item_stats(&t, &mut FakeGame::default(), &mut it, Some(&mut rq), false).unwrap();
        let rolled = Seed::init_low(9).roll(50) as i32 + 10;
        let expect = if replaced { ovr } else { rolled };
        assert_eq!(it.stats.base(stat::GOLD, 0), expect, "override {ovr}");
    }
}

/// `generation.md` §4 r3 (Other): spawn stack equal to min stack keeps hi
/// = spawn stack (only `<` or 0 falls back), so r(0) draws nothing.
// Covers: specs/items/generation.md §4 r3
#[test]
fn misc_stack_spawnstack_equal_to_min() {
    let mut t = tables();
    let mut r = item_rec(RING, b"rin ");
    r.stackable = 1;
    r.minstack = 5;
    r.spawnstack = 5;
    r.maxstack = 20;
    let i = push_item(&mut t, r);
    let mut it = item(i, 1);
    it.unit_seed = Seed::init_low(4);
    init_item_stats(&t, &mut FakeGame::default(), &mut it, None, false).unwrap();
    assert_eq!(it.stats.base(stat::QUANTITY, 0), 5);
    assert_eq!(it.unit_seed, Seed::init_low(4), "r(0) draws nothing");
}

/// Staff tables with skills whose `itypea1` cycles through a fitting type,
/// a non-fitting type, 0 (always), 1 (a type the staff is not) and −1
/// (< 1: always).
fn staff_tables() -> (ItemTables, usize) {
    let mut t = tables();
    t.itemtypes[ty::STAF as usize].staffmods = 1;
    let i = push_item(&mut t, item_rec(ty::STAF, b"sst "));
    t.skill_lists.counts[1] = 30;
    t.skill_lists.max = 30;
    t.skill_lists.lists = vec![0; 7 * 30];
    t.skill_lists.lists[30] = 36;
    t.skills = (0..80)
        .map(|k| SkillRec {
            itypea1: [ty::STAF as i16, ty::HELM as i16, 0, 1, -1][k % 5],
            reqlevel: 1,
            maxlvl: 20,
        })
        .collect();
    (t, i)
}

/// `generation.md` §6.2 steps 2–5, written from the spec.
fn staffmods_model(
    t: &ItemTables,
    rec: usize,
    rq: &ItemRequest,
    format: u16,
    quality: u8,
    mut s: Seed,
) -> (Seed, BTreeMap<(u16, u16), i32>) {
    let mut out = BTreeMap::new();
    let pct = |s: &mut Seed| (s.step() % 100) as i32;
    let bonus = if rq.flags2 & 0x20 != 0 { rq.ilvl } else { 0 };
    let v = pct(&mut s) + bonus;
    let count = match v {
        v if v > 90 => 3,
        v if v > 70 => 2,
        v if v > 30 => 1,
        _ if bonus != 0 => 1,
        _ => return (s, out),
    };
    let l = rq.ilvl;
    let base: i32 = match () {
        _ if l > 36 && format >= 100 => 5,
        _ if l > 24 => 4,
        _ if l > 18 => 3,
        _ if l > 11 => 2,
        _ => 1,
    };
    let mut chosen = Vec::new();
    for _ in 0..count {
        let p = pct(&mut s);
        let mut tier = match p {
            p if p > 80 => base + 1,
            p if p > 30 => base,
            p if p > 10 => base - 1,
            _ => base - 2,
        };
        tier = tier.max(1);
        if quality == q::LOW && tier > 3 {
            tier = 4;
        }
        let mut skill = 0u16;
        for _ in 0..6 {
            skill = 36 + (5 * (tier - 1)) as u16 + (s.step() % 5) as u16;
            let fits = t
                .skills
                .get(usize::from(skill))
                .is_none_or(|r| r.itypea1 < 1 || t.is_type(rec, r.itypea1));
            if fits && !chosen.contains(&skill) {
                chosen.push(skill);
                break;
            }
        }
        let value = if format < 100 || quality != q::LOW {
            match pct(&mut s) + bonus / 2 {
                v if v >= 90 => 3,
                v if v >= 60 => 2,
                _ => 1,
            }
        } else {
            1
        };
        out.insert((stat::ITEM_SINGLESKILL, skill), value);
    }
    (s, out)
}

/// `generation.md` §6.2: every threshold at its boundary (ilvl 11/12,
/// 18/19, 24/25, 36/37; format 100; enough seeds for every pct value),
/// and the `itypea1` < 1 test, against the spec model.
// Covers: specs/items/generation.md §6.2 r3, §6.2 r4, §6.2 r5
#[test]
fn staffmods_boundaries() {
    let (t, i) = staff_tables();
    // Six seeds for every first pct value 0–99.
    let seeds: Vec<u32> = (0..100)
        .flat_map(|p| {
            (0u32..)
                .filter(move |&x| Seed::init_low(x).step() % 100 == p)
                .take(6)
        })
        .collect();
    let mut first_pcts = std::collections::BTreeSet::new();
    for ilvl in [11, 12, 18, 19, 24, 25, 36, 37] {
        for flags2 in [0, req::STAFFMODS_ILVL] {
            for format in [2u16, 100, 101] {
                for quality in [q::LOW, q::NORMAL] {
                    for &seed in &seeds {
                        let rq = ItemRequest {
                            ilvl,
                            flags2,
                            ..Default::default()
                        };
                        let mut it = item(i, seed);
                        it.format = format;
                        it.quality = quality;
                        class_skill_mods(&t, &mut it, &rq);
                        let (s, want) =
                            staffmods_model(&t, i, &rq, format, quality, Seed::init_low(seed));
                        let ctx = (ilvl, flags2, format, quality, seed);
                        assert_eq!(it.item_seed, s, "{ctx:?}");
                        let got = it
                            .stats
                            .lists
                            .get(&ListKey::ITEM)
                            .cloned()
                            .unwrap_or_default();
                        assert_eq!(got, want, "{ctx:?}");
                        first_pcts.insert(Seed::init_low(seed).step() % 100);
                    }
                }
            }
        }
    }
    assert_eq!(first_pcts.len(), 100, "every first pct value is exercised");
}

/// Socket helm: 3 × 4 inventory, 6 gem sockets, maxsock 6 at every ilvl.
fn big_socket_helm(t: &mut ItemTables) -> usize {
    let mut r = item_rec(ty::HELM, b"cap ");
    r.hasinv = 1;
    r.gemsockets = 6;
    r.invwidth = 3;
    r.invheight = 4;
    let h = &mut t.itemtypes[ty::HELM as usize];
    (h.maxsock1, h.maxsock25, h.maxsock40) = (6, 6, 6);
    push_item(t, r)
}

/// `generation.md` §7.3: quality caps magic 4, rare 2, crafted and
/// tempered 3; other qualities keep the w × h cap.
// Covers: specs/items/generation.md §7.3
#[test]
fn socket_count_quality_caps() {
    let mut t = tables();
    let i = big_socket_helm(&mut t);
    for (quality, want) in [
        (q::NORMAL, 6),
        (q::SUPERIOR, 6),
        (q::MAGIC, 4),
        (q::RARE, 2),
        (q::CRAFTED, 3),
        (q::TEMPERED, 3),
    ] {
        let mut it = item(i, 1);
        it.quality = quality;
        socket_count(&t, &mut it, 6);
        assert_eq!(
            it.stats.base(stat::NUMSOCKETS, 0),
            want,
            "quality {quality}"
        );
    }
}

/// `generation.md` §7.1 r3: only format < 100 body armor stops before the
/// draw; format 100 draws. r7: p < 33 sockets, p = 33 does not.
// Covers: specs/items/generation.md §7.1 r3, §7.1 r7
#[test]
fn socket_roll_boundaries() {
    let mut t = tables();
    let mut r = item_rec(ty::TORS, b"qui ");
    r.hasinv = 1;
    r.gemsockets = 4;
    r.invwidth = 2;
    r.invheight = 3;
    let tors = push_item(&mut t, r);
    let tt = &mut t.itemtypes[ty::TORS as usize];
    (tt.maxsock1, tt.maxsock25, tt.maxsock40) = (4, 4, 4);
    let game = FakeGame::default();
    let rq = ItemRequest::default();
    for (format, draws) in [(99u16, false), (100, true)] {
        let mut it = item(tors, 5);
        it.format = format;
        it.quality = q::NORMAL;
        socket_roll(&t, &game, &mut it, &rq);
        assert_eq!(it.item_seed != Seed::init_low(5), draws, "format {format}");
    }
    for (p, want) in [(32, true), (33, false)] {
        let seed = find_seed(|s| s.roll(100) == p);
        let mut it = item(tors, seed);
        it.quality = q::NORMAL;
        socket_roll(&t, &game, &mut it, &rq);
        assert_eq!(it.flags & flag::SOCKETED != 0, want, "p {p}");
    }
}

fn forced_rq(i: usize, flags1: u32) -> ItemRequest {
    ItemRequest {
        item: i as i32,
        format: 101,
        force: true,
        quality: q::NORMAL,
        flags1,
        ..Default::default()
    }
}

/// `generation.md` §9 r1, r2: a forced request copies exactly the named
/// bits from flags1; other flags stay as creation set them.
// Covers: specs/items/generation.md §9 r1
#[test]
fn forced_flag_copies() {
    let mut t = tables();
    let i = push_item(&mut t, item_rec(RING, b"rin "));
    let copied = flag::IDENTIFIED | flag::NOSELL;
    for flags1 in [0, copied, copied | flag::ETHEREAL | flag::NAMED] {
        let mut rq = forced_rq(i, flags1);
        let c = create_item(
            &t,
            &mut FakeGame::default(),
            &mut rq,
            false,
            FakeStats::default(),
            0,
        )
        .unwrap();
        assert_eq!(
            c.item.flags,
            flag::INIT | flags1 & copied,
            "flags1 {flags1:#x}"
        );
    }
}

/// `generation.md` §9 r5: a forced ear takes the name, ear level and the
/// 0x8000 bit of flags1; a player's ear takes the client's hardcore flag.
/// Other flags are untouched.
// Covers: specs/items/generation.md §9 r5
#[test]
fn ear_named_flag_copy() {
    let mut t = tables();
    let i = push_item(&mut t, item_rec(ty::PLAY, b"ear "));
    let base = flag::INIT | flag::EAR;
    for flags1 in [0, flag::NAMED] {
        let mut rq = forced_rq(i, flags1);
        rq.name = *b"forced-ear-name\0";
        rq.ear_level = 33;
        let c = create_item(
            &t,
            &mut FakeGame::default(),
            &mut rq,
            false,
            FakeStats::default(),
            0,
        )
        .unwrap();
        assert_eq!(c.item.flags, base | flags1, "flags1 {flags1:#x}");
        assert_eq!(c.item.name, rq.name);
        assert_eq!(c.item.ear_level, 33);
    }
    for (hc, named) in [(Some(true), flag::NAMED), (Some(false), 0), (None, 0)] {
        let mut rq = ItemRequest {
            item: i as i32,
            format: 101,
            quality: q::NORMAL,
            unit: Some(RequestUnit {
                class: 3,
                player: Some(PlayerInfo {
                    name: *b"player-name\0\0\0\0\0",
                    level: 12,
                    hardcore: hc,
                }),
            }),
            ..Default::default()
        };
        let c = create_item(
            &t,
            &mut FakeGame::default(),
            &mut rq,
            false,
            FakeStats::default(),
            0,
        )
        .unwrap();
        assert_eq!(
            c.item.flags,
            base | flag::INSTORE | named,
            "hardcore {hc:?}"
        );
        assert_eq!(c.item.ear_level, 12);
    }
}

fn run_prop(t: &ItemTables, it: &mut Item<FakeStats>, r: PropRec) {
    use crate::items::props::{apply_property, PropCtx};
    apply_property(t, it, &mut PropCtx::item(0), &r);
}

/// `properties.md` §4.1: max < min swaps the bounds (min 15, max 5 →
/// 5 + roll(11)), on several seeds.
// Covers: specs/items/properties.md §4.1
#[test]
fn value_roll_swaps_bounds() {
    use crate::items::props::roll_value;
    for seed in 0..20 {
        let mut s = Seed::init_low(seed);
        let want = 5 + Seed::init_low(seed).roll(11) as i32;
        assert_eq!(roll_value(&mut s, 15, 5), want, "seed {seed}");
    }
}

/// `properties.md` §4.2: with set ≠ 0, stat 58 sets stat 326 to 1 only
/// when it is 0; a set of another stat leaves 326 alone.
// Covers: specs/items/properties.md §4.2
#[test]
fn poison_count_on_set() {
    let mut t = tables();
    let i = push_item(&mut t, item_rec(RING, b"rin "));
    let mut p58 = prop1(1, stat::POISONMAXDAM);
    p58.slots[0].set = 1;
    let mut p57 = prop1(1, 57);
    p57.slots[0].set = 1;
    t.properties = vec![p58, p57];
    let mut it = item(i, 1);
    run_prop(&t, &mut it, rec(0, 0, 4, 4));
    assert_eq!(it.stats.item_list(stat::POISONMAXDAM, 0), 4);
    assert_eq!(it.stats.item_list(stat::POISON_COUNT, 0), 1);
    let mut it = item(i, 1);
    it.stats.list_set(ListKey::ITEM, stat::POISON_COUNT, 0, 5);
    run_prop(&t, &mut it, rec(0, 0, 4, 4));
    assert_eq!(it.stats.item_list(stat::POISON_COUNT, 0), 5);
    let mut it = item(i, 1);
    run_prop(&t, &mut it, rec(1, 0, 4, 4));
    assert_eq!(it.stats.item_list(57, 0), 4);
    assert!(!it.stats.lists[&ListKey::ITEM].contains_key(&(stat::POISON_COUNT, 0)));
}

/// `properties.md` §4.3: the damage base resets apply to weapons only; a
/// non-weapon with damage columns keeps its base stats (function 7).
// Covers: specs/items/properties.md §4.3
#[test]
fn base_reset_damage_weapon_only() {
    let mut t = tables();
    let mut r = item_rec(ty::HELM, b"cap ");
    (r.mindam, r.maxdam, r.mindam2, r.maxdam2) = (3, 7, 4, 8);
    let i = push_item(&mut t, r);
    t.properties = vec![prop1(7, stat::MAXDAMAGE_PERCENT)];
    let mut it = item(i, 1);
    run_prop(&t, &mut it, rec(0, 0, 20, 20));
    for s in [21, 22, 23, 24] {
        assert_eq!(it.stats.base(s, 0), 0, "stat {s}");
    }
    assert_eq!(it.stats.item_list(stat::MAXDAMAGE_PERCENT, 0), 20);
}

/// `properties.md` §5 r4: with `max` = 0 the level is capped at the
/// skill's `maxlvl` when it is ≥ 1 (here 1), at 20 only when < 1.
// Covers: specs/items/properties.md §5 r4
#[test]
fn skill_event_level_cap_one() {
    let mut t = tables();
    let i = push_item(&mut t, item_rec(RING, b"rin "));
    t.properties = vec![prop1(11, 195)];
    t.skills = vec![SkillRec {
        itypea1: 0,
        reqlevel: 1,
        maxlvl: 1,
    }];
    let mut it = item(i, 1);
    it.ilvl = 50;
    run_prop(&t, &mut it, rec(0, 0, 7, 0));
    assert_eq!(
        it.stats.lists[&ListKey::ITEM],
        BTreeMap::from([((195, 1), 7)])
    );
}
