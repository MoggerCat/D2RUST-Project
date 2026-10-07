# cov-core: coverage session note

Branch `claude/cov-core` (base `claude/specs-staging-7` @ 1633f67). Toolchain note: the pinned 1.99.0 would not install in this container (concurrent rustup installs), so work and gate ran on `stable` (1.97) via `+stable` / `RUSTUP_TOOLCHAIN=stable`; nextest not installed, `cargo test` used.

## Rules covered (units covered, before -> after, `tools/coverage.py`)

| spec | before | after |
|---|---|---|
| combat/damage | 119/138 | 131/138 |
| combat/events | 20/38 | 38/38 |
| combat/hit | 50/67 | 66/67 |
| combat/vitals | 49/55 | 51/55 |
| data/calc-expressions | 42/50 | 44/50 |
| data/fixups | 44/46 | 46/46 |
| data/loading | 24/50 | 30/50 |
| data/patch-layers | 16/22 | 17/22 |
| items/generation | 70/104 | 74/104 |
| items/affixes | 45/64 | 48/64 |
| items/quality | 45/64 | 46/64 |
| sim/rng | 30/68 | 37/68 |
| sim/stat-lists | 82/88 | 88/88 |
| sim/units | 51/88 | 70/88 |
| sim/pets | 32/33 | 33/33 |
| sim/stats | 37/38 | 37/38 |

All covered units are `unit` tier (unverified, M02).

## Code fixes found by tests (spec is the reference)

- damage §5.3 r7: `leech` with no attacker left the shifted fields (`x << 6`); spec stores `(x<<6)/64` = x. Fixed in `combat/damage.rs`.
- affixes §1 r4: `group_taken` read suffix slot 0 of `scro`/`book` items as an affix id; skipped now (`items/affixes.rs`).
- generation §9 r2: format-0 forced socket step missing from `forced()` (`items/create.rs`); added.
- units §3.1 r9: allocation with flags bit 1 clear returned `Err(NotAdded)`; spec returns the unlinked unit. `lifecycle::allocate` changed, `NotAdded` removed, two old assertions that contradicted the spec updated.
- Not implemented though sort marked A (now added, not yet called by wiring): stat-lists §8.9 `remove_temporary_lists`; pets §10 `create/free_all/player_death/resync_max` (`PetLifecycleWorld`).
- New pure-rule code (B): `combat/range.rs` (hit §7.1 hostility, §7.2 in-melee-range, §7.3 melee range) over `RangeWorld`; units §4.7 `anim_rate.rs`, §6.6 `event_records.rs`, §6.5 `replenish.rs`; generation §10.1 `find_code`, §10.2 `create_from_index`.
- IMPORTANT finding: the real hosts' `Pending::may_attack`, `melee_range`, `in_melee_range` default to `false` (wiring/action/pending.rs); `combat/range.rs` is the rule but nothing calls it yet (C: implement `RangeWorld` on the world and answer the seams).
- Test-support: `BodyFake.srvdo_result` field added (bodies/fake.rs).

## Rules left, with reason

- damage §7.1 text, r1, r2, r4, r5, r6 (reaction branches): bucket C, NOT implemented (the `Pending::reaction` seam is an empty default; the player/monster mode requests, soft hits, AI state 19, dodge/avoid/evade/block/death forms need new wiring). Needs Opus. (§7.1 r3 covered.)
- damage §2 r1 (melee_result stored by the skill do-function): call-site rule in skills/use; damage §8 covered by dispatch test.
- vitals §4.8 text, r1, r2 (DT/DD start): C, not implemented (only penalties/corpse exist); vitals §5.1 r4 (PlayerPos registry value, d2rs does not run it): nothing to code.
- hit §7.4: X (prose caller list).
- data: calc policy r3 (layered rebuild); patch-layers §1 (no Original/Mod pipeline orchestrator), edge r1 (needs local run of `edge_duplicate_columns_in_1_14d_headers`); loading §1 r3 (`expfield.d2` not implemented); loading §9 claimed on a synthetic test, the 1.14d sizes are in a new ignored test `combined_index_space_sizes` (tests/game_data.rs) needing a local run; X: loading -txt mode, rules text, policy r5-6.
- items: generation §10.3/§10.4 (start items) and §12.1-12.3 (repair/recharge/runeword removal) C (missing inventory/charstats/ItemStats seams); X: generation §11, affixes §12, quality §2, §8 r1, §10, edge r5.
- sim: units §3.3/§3.4 (18 units) C (per-room inactive store, serialization); units §4.5 (position forms, GH -1 not modelled); units §8, edge r4, stats edge r5, rng §3 r6, §5.5, §6 X; rng §5.5 row1-2 client globals; rng §7 per-system seed ownership needs wiring-level seed-identity tests. The wiring of `anim_rate`, `event_records`, `replenish` into `UnitHooks` is open.
## Gate

`RUSTUP_TOOLCHAIN=stable sh tools/gate.sh --no-client`: all tools steps, fmt, depcheck, clippy PASS; doc-tests PASS. FAIL in test d2-sim (2) and test rest (3), identical on the untouched base 1633f67: `world::objects::tests::{routes_match_function_table, route_check_catches_perturbations}` (d2-sim) and d2-server `world_data_tables` (3 tests, synthetic install "not an MPQ archive"). d2-client not built (disk; client untouched).
