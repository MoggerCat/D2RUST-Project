// Spec: specs/monsters/init.md §5, §10, §16.2, §18; specs/monsters/population.md §2.4, §2.5, §6.4 (init ↔ units, stats, AI and population regions)
//! Monster init on the real unit records, AI store and population
//! regions, reached through population's seams.

use super::population::{isle, monsters};
use super::*;
use crate::monsters::init::type_flag;
use crate::monsters::population::{preset, MonsterInit};

/// Monster class 0 with two component choices (3 and 2 pieces): its
/// region entries get appearance variants (`population.md` §2.4).
fn composite(t: &mut WorldTables) {
    let m2 = &mut t.pop.monstats2[0];
    m2.components[0] = 3;
    m2.components[1] = 2;
    m2.composit_total = 2;
    m2.total_pieces = 3;
}

// Covers: specs/monsters/init.md §5 r4, §10 text, §10 r1; specs/monsters/population.md §2.5 text, §2.5 r2
#[test]
fn components_come_from_the_region_entry_on_the_unit_seed() {
    let mut fx = Fx::with_tables(isle_ds1s(), composite);
    let (a, _) = isle(&mut fx);
    fx.sim.create_regions();
    let entry = {
        let r = fx.sim.world.pop.regions.get(ISLE as i32).unwrap();
        assert_eq!(r.entries[0].class, 0);
        r.entries[0]
    };
    assert!(entry.variant_count > 1);
    fx.sim
        .population(&mut fx.game, |cx| preset::place_presets(cx, a));
    fx.assert_clean();
    let u = monsters(&fx)[0];
    // The type init's first unit-seed draw picks the variant.
    let rec = fx.sim.action.sys.units.get(u).unwrap();
    let i = Seed::init_low(rec.init_seed).roll(i32::from(entry.variant_count));
    let md = fx.sim.world.monsters.get(u).unwrap();
    assert_eq!(md.components, entry.variants[i as usize]);
    // The found entry is not added again.
    let r = fx.sim.world.pop.regions.get(ISLE as i32).unwrap();
    assert_eq!(r.entry_count, 1);
}

// Covers: specs/monsters/init.md §16.2 r1, §16.2 r2, §16.2 r3, §16.2 r4, §18 r2; specs/monsters/population.md §6.4
#[test]
fn champion_pack_member_counts_in_the_real_region() {
    let mut fx = Fx::new(isle_ds1s());
    let (a, _) = isle(&mut fx);
    fx.sim.create_regions();
    fx.sim
        .population(&mut fx.game, |cx| preset::place_presets(cx, a));
    let u = monsters(&fx)[0];
    let bosses = fx.sim.world.pop.regions.get(ISLE as i32).unwrap().bosses;
    let seed = fx.sim.action.sys.units.get(u).unwrap().seed;
    // Population's modifier 16 call (`0x005A48C0`), with the regions it
    // holds.
    fx.sim
        .population(&mut fx.game, |cx| cx.host.add_modifier(u, 16, cx.state));
    fx.assert_clean();
    let md = fx.sim.world.monsters.get(u).unwrap();
    assert!(md.has_flag(type_flag::BOSS | type_flag::CHAMPION));
    assert!(md.has_flag(type_flag::UNIQUE));
    assert!(md.has_umod(16));
    // `0x005A0320`: the boss counter of the monster's region.
    assert_eq!(
        fx.sim.world.pop.regions.get(ISLE as i32).unwrap().bosses,
        bosses + 1
    );
    // §18 step 2 ran umod 1 (the name seed draw) on the unit seed.
    let mut s = seed;
    let v = s.step() as u16;
    assert_eq!(md.name_seed, v);
    // A second call does nothing (type flag 4).
    fx.sim
        .population(&mut fx.game, |cx| cx.host.add_modifier(u, 16, cx.state));
    assert_eq!(
        fx.sim.world.pop.regions.get(ISLE as i32).unwrap().bosses,
        bosses + 1
    );
}
