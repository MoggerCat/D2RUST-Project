# Handoff: game-file tests for the item and treasure specs (branch `claude/game-tests-items-treasure`, 2026-10-06)

> To be folded into `docs/HANDOFF.md` (§1, §4, §5) and `docs/PLAN.md` by a docs session; this file stays as the detailed record.

Cloud test session, repo only (no `game/`), medium effort, from
`claude/tender-meitner-mphas3` at `4b5b0bf`. Read: `specs/items/*.md`,
`docs/`, `crates/`. No spec, `d2-sim` source, `HANDOFF.md` or `PLAN.md`
edit; only the files below.

## 1. What was added

| File | Contents |
|---|---|
| `crates/d2-sim/tests/items_treasure_live/mod.rs` | shared loader: the live `.bin` set of `D2_GAME_DIR`, loaded and fixed up as `d2-data`'s `fixups_on_live_set` does (`bin::load`, `fixup::read_animdata`, `fixup::apply`), once per test binary; `typed::<T>()` decodes one table |
| `crates/d2-sim/tests/game_treasure.rs` | 16 `#[ignore]` tests: every real 1.14d vector of `treasure.md` "Test vectors", its stated table facts, 6 sweeps |
| `crates/d2-sim/tests/game_items.rs` | 8 `#[ignore]` tests: the 1.14d table facts the four item specs state, 4 sweeps. The item specs have **no** real vectors yet (`generation.md` "Real 1.14d vectors need the recording in Open questions 2"; `affixes.md`, `quality.md`, `properties.md` list synthetic vectors only, all already unit tests) |

`ItemTables::from_fixed` and `TreasureClasses::build` are the projections
the sim uses; nothing in the tests restates table values except the ones
the specs state. No expected value is invented: each is a number a spec
states, or an invariant a spec rule implies (named per test below).

### game_treasure.rs

| Test | Checks (spec source) | Claim (game tier) |
|---|---|---|
| `live_tc_counts_and_kinds` | vector "TC count, kinds": 1,013 TCs (array count, TC 0 included: OQ4 "1,013 × 0x2C"), no load note (0 misses, no forward reference §1.2); §1.3: 160 automatic TCs, 763 entries, 85 with classic total 0, 44 with expansion total 0; §1.5 over the `treasureclassex` TCs (index ≥ 161): 2,742 TC / 660 item / 2 unique / 0 set entries, 81 `mul` all in {1280, 1536, 2048}, no entry mods (no other key), 184 negative picks, 300 nonzero NoDrop, 240 nonzero mods | §1.3 text, §1.5 text |
| `live_automatic_tcs` | §1.3: itemtypes with `treasureclass` ≠ 0 are exactly 27 `bow`, 45 `weap`, 46 `mele`, 50 `armo`, 85 `abow`; 38 is `tpot`; `A` = 5; TCs 1–160 are `<code><Lv>`, group 0, level `Lv` − 3, picks 1, nodrop 0, mods 0 | §1.3 text |
| `live_act1_h2h_a` | vector `Act 1 H2H A`: TC 430, group 12, picks 1, nodrop 100, `gld` (item 523) 21, TCs 218 / 203 / 370 (by name) 16 / 21 / 2, starts 0, 21, 37, 58, totals 60 / 60 | §1.4 |
| `live_rop_n` | vector `ROP (N)`: TC 855 `Diablo (N)` prob 4, `Annihilus` flags 0x11 row 381 prob 1 (id = item index of its code), totals 4 / 5 | §1.5 r4 |
| `live_act1_champ_a` | vector `Act 1 Champ A`: picks −2, `Act 1 Citem A` 1, `Act 1 Cpot A` 2 | §1.4 |
| `live_tcx_slots_5_6_zero` | §1.4 table: `treasureclassex` bytes +0x30…+0x33 are 0 in every record (d2rs stores 0 without reading them) | — |
| `live_chest_table` | §1.6: all 45 chest TCs found with the right names; `Act 1 Chest A` = 385 | §1.6 |
| `live_get_by_level` | vectors `get(430, 40)` = 445 `Act 1 (N) H2H B` (levels 38, 40, 41 at 444–446, same group), `get(430, 0)` = 430, `get(430, 85)` = 471 `Act 5 (H) H2H C`, last of its group, `get(0, 40)` none | §2 |
| `live_chest_tiers` | vectors "chest tier": Normal act 0 area levels 1 (level 2), 12 (level 37), 1 (level 8); tiers 0 (level 8), 2 (level 37); TC 385; Hell act 4 area levels 0 (109), 83 (136) | §4 r2, §4 r4 |
| `live_nodrop_pairs` | §5.4: 23 expansion / 15 classic distinct (nodrop, total) pairs (nodrop ≠ 0, total > 0: a zero total ends the slot before step 5); for each and `n` = 2…8, `nodrop()` equals floor(C·n0^n / ((n0 + C)^n − n0^n)) in u128, the form the spec states is a valid check | §5.4 r5 |
| `live_ratio_rows_are_version_1` | §6 step 3 / Edge case 6: the ratio row is a `Version` 1 row for all four (`Class Specific`, `Uber`) pairs | — |
| `sweep_drop_quality_every_item` | §6 on every item × every live TC mod set × `L` 0, 1, 2, 30, 60, 99 × `M` −100, 0, 100, 1000: no error, quality 1–7; step 2 results exact and drawless (`normal` → 2, `unique` / `magic`+`quest` → 7); ladder gates kept (6 only with itemtypes `rare`; ≥ 4 with `magic`); `M` ≤ −100 → ≤ 3 | §6 r2 |
| `sweep_tc_structure` | every TC: picks ≠ 0; first starts 0; expansion starts strictly rise (no entry with prob < 1), classic starts never fall; totals ≥ the last start; TC entries point to an earlier TC ≥ 1; item entries to a real item; unique / set entries expansion only | — |
| `sweep_every_tc_resolves_and_picks` | every TC: `get(i, lvl)` resolves for `lvl` 0…120 within its group; every `r` < total selects an entry (expansion: `start ≤ r <` next start; classic: an entry without 0x10, `start ≤ r`) | — |
| `sweep_walk_every_tc` | §5 from every TC × classic / expansion × `S` 1 / 8 × `M` 0 / 500 × `L` 1 / 50 / 99 × 2 seeds: no fatal error (incl. `NoDropRange`, OQ5), ≤ 6 items, real item ids, quality 1–7, drop flags 0x04 / 0x10 never set (§5.7 step 4: slot mods 5, 6 are 0), classic: no item with `version` ≥ 100 and no unique / set index | — |
| `sweep_monster_tcs_resolve` | §3.2 / §3.3 for every monstats row × difficulty × rank (normal, champion, unique, superunique without record) × quest open / closed, and every superuniques row: TC 0 or a TC that `get` resolves for every level | — |

### game_items.rs

| Test | Checks (spec source) | Claim (game tier) |
|---|---|---|
| `live_affix_parts` | `affixes.md` §1 r1: suffixes 0–746, prefixes 747–1,415, automagic 1,416–1,451 (747 / 669 / 36 rows); id = combined index + 1 | `affixes.md` §1 r1 |
| `live_qualityitems_count` | `quality.md` §7 r1, edge case 5: 8 qualityitems rows | — |
| `live_unique_rarity_32_bits` | `quality.md` edge case 4: bytes +0x32/+0x33 of every uniqueitems record are 0, so the 32-bit read equals `rarity` | `quality.md` §edge-cases-original-bugs r4 |
| `live_type_numbers` | `generation.md` §1.3 and `quality.md` §7.1: the 24 itemtypes row indices named there have the named codes | — |
| `sweep_create_every_item_every_quality` | `generation.md` §3 on every item × request quality 0–9 × ilvl 1 / 30 / 60 / 99 × Normal / Hell × expansion / classic (fresh game seed per item): classic refuses exactly `version` ≥ 100 (§3 r1); every other creation succeeds (`quality.md` §4: the downgrade chain ends in normal, which cannot fail; a `Fatal` here, e.g. the charm exit of `affixes.md` edge case 4, is reported as a failure); invariants: quality 1–9; unique row has the item's code, or no row only on an items `unique` base (§8 r4, r6); set row matches code, `lvl` ≤ ilvl, set ≠ 29 (§9 r1); superior row < 8 (< 4 throwable / `nodurability`, §7 r1); low row < lowqualityitems count; affix slots hold ids of their part that fit (§4.1 r3–r4), no two share a group (§3 r4.9), no expansion affix on classic; rare names in their parts; auto affix only expansion, not set / unique, from the automagic part on an `auto prefix` base (§4 r5.3); no ethereal in classic; qualities 1–3: sockets ≤ min(w × h, 6) and ≤ `gemsockets` | `generation.md` §3 r1 |
| `sweep_forced_every_unique` | `quality.md` §8 r2: a forced request on every uniqueitems row whose code is an item gives quality 7 with that file index (quality 2 when the base's itemtype has `normal`, §4 r3.4) | — |
| `sweep_preferred_every_set_item` | `quality.md` §9 r1–r2: preferring every setitems row (index row + 1, ilvl 99, flags2 0x01) picks it (or §4 r3 overrides: `normal` → 2, items `unique` → 7) | — |
| `sweep_every_affix_pick` | `affixes.md` §3 on every item (format 101, quality magic) × prefix / suffix / automagic (when the base has `auto prefix`) × each distinct (alvl, sockets allowed) over ilvl 1–99, forced past the coin: a pick is an id of its part passing every step 4 test (spawnable, level window, socket clause §4.1 r2, fit, group, frequency, class); 0 only when no row passes (step 5) | — |

Claims: 13 rule units at the game tier (`treasure.md` 10: §1.3 text,
§1.4, §1.5 text, §1.5 r4, §1.6, §2, §4 r2, §4 r4, §5.4 r5, §6 r2;
`affixes.md` §1 r1; `quality.md` edge r4; `generation.md` §3 r1), each
on a test that checks the rule's outcome on the live data. Sweeps checking
invariants only, and table-fact tests whose rule is wider than the fact,
claim nothing. `py tools/coverage.py --summary` total: game 187 → 200
units, verified 217 → 230, any 2,459 → 2,460 (`treasure.md` game 0 → 10,
`affixes.md` / `generation.md` / `quality.md` 0 → 1 each). Per
`docs/COVERAGE.md` §3 these count as verified only once the local run
below passes; until then read them as queued.

## 2. Local run queue (for `docs/HANDOFF.md` §5 C)

```
D2_GAME_DIR=<install> cargo test -p d2-sim --test game_treasure -- --ignored
D2_GAME_DIR=<install> cargo test -p d2-sim --test game_items -- --ignored
```

Expect: `game_treasure` 16 passed, `game_items` 8 passed. By name:
`live_tc_counts_and_kinds live_automatic_tcs live_act1_h2h_a live_rop_n
live_act1_champ_a live_tcx_slots_5_6_zero live_chest_table
live_get_by_level live_chest_tiers live_nodrop_pairs
live_ratio_rows_are_version_1 sweep_drop_quality_every_item
sweep_tc_structure sweep_every_tc_resolves_and_picks sweep_walk_every_tc
sweep_monster_tcs_resolve` and `live_affix_parts live_qualityitems_count
live_unique_rarity_32_bits live_type_numbers
sweep_create_every_item_every_quality sweep_forced_every_unique
sweep_preferred_every_set_item sweep_every_affix_pick`. Add `--release` if
the sweeps are slow in a debug build (they run ~10^5 creations and ~5·10^4
walks).

A failure is a finding for the owner spec (or the d2rs module), not a
reason to change the numbers (`docs/HANDOFF.md` §5). Interpretation points
to look at first if one fails:

- `live_tc_counts_and_kinds`: 1,013 is read as the array count with TC 0
  (Open question 4 "1,013 × 0x2C"); 660 item entries as the
  `treasureclassex` TCs only (§1.5; the automatic TCs hold 763 more);
  184 / 300 / 240 over the same TCs (the automatic ones have picks 1,
  nodrop 0, mods 0, so the scope does not change them).
- `live_nodrop_pairs`: pairs with a total of 0 are excluded (they never
  reach §5.4 step 5); if the counts differ by one, check that scope first.
- The sweeps print the first 20 failures with item / TC, quality, level,
  difficulty and mode: each is a spec question (§7 of `HANDOFF.md`) unless
  the d2rs code contradicts its spec.

## 3. Gate (this branch)

`cargo fmt --check`; `cargo clippy --workspace --all-targets -- -D
warnings`; `cargo test -p d2-sim` (the 24 new tests are ignored; the rest
unchanged); `cargo run -p depcheck`; `python3 tools/spec_index.py
--check`; `python3 tools/methods.py check`; `python3 tools/coverage.py
--check` (3,274 claims, 0 errors) and `--selftest` (ok). M08 is not
shown: the tests cannot run without game files; the local run is their
first execution.

## 4. Signature changes

None. The tests use public `d2_sim::items` / `d2_sim::treasure` /
`d2_data` APIs only; no crate source changed.
