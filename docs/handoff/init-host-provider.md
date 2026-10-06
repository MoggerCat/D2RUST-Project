# InitHost provider (HANDOFF §2 step 7t(e))

Branch `claude/init-host-provider`, from `claude/tender-meitner-mphas3`
at `01dff69`. Cloud implementation session, medium effort. Inputs:
`specs/monsters/init.md` (§3, §5, §6 step 13, §12, §17.3, §18),
`specs/data/runtime-maps.md` §2, `monsters/init/seams.rs` docs,
`docs/handoff/mutants-monsters-missiles.md`.

## What changed

**Provider** (`wiring/worldgen/init_units.rs`, `WorldHost`'s `InitHost`):

| Method | Before | Now |
|---|---|---|
| `level_id`, `region_variant_count`, `region_variant`, `minions` | wired (earlier session) | unchanged; now tested directly (`init_host_answers_from_the_wired_world`) |
| `montype_is` | default `montype == ty` | **wired**: bit (montype, ty) of the montype equivalence matrix (`runtime-maps.md` §2), new field `WorldTables::montype_equiv` |
| `has_inventory`, `has_item_at` | default `false` | **pending (kept)**: spec text exists (unit +0x60, §6 step 13, §12), but no monster inventory exists in this wiring (`new_inventory` is pending, its provider is items / vendors). "No inventory" is what the wired world holds; wire all three together. |
| `place` | default `None` | **pending (kept)**: placement is `population.md` §9, but in this wiring every monster is created by population's own `0x005B2A00` (`population_init`); `init::create` (the only caller of `place`) is never called. A provider needs a point-only split of `population::placement::place` (it creates in the same call) and `allocate` wired with it. |

`WorldTables` gained `montype_equiv: EquivMatrix` (default empty, so
`montype_is` answers false until set). The two game-file tests that
build `WorldTables` field by field (`d2-server/tests/game_wired_host.rs`,
`game_world_data.rs`) set it from `live().fixed.montype_equiv`; the
other builders use `..WorldTables::default()`.

**Tests** (7 new):
- `wiring/worldgen/tests/init.rs`: `montype_nesting_reads_the_montype_matrix`
  (nested yes, parent-of-child no, row / column 0 and out-of-range no,
  which plain equality would answer differently),
  `init_host_answers_from_the_wired_world` (level of the unit's room,
  0 with no room; minion list in link order; no inventory),
  `init_host_state_reaches_the_action_systems` (base stats round-trip
  through the stat lists; difficulty 2 reaches init, population and AI
  info; alignment reaches its pending provider).
- `monsters/mutant_tests/init_seams.rs` (mounted from `init/tests.rs`):
  a `Bare` host with only the required methods pins every defaulted
  answer (`default_queries_answer_nothing`,
  `default_creation_seams_refuse`, `default_montype_is_plain_equality`)
  and `create_on_the_default_seams_places_nothing` (no point → no unit,
  also for a probe). These are the default bodies' contract
  (`seams.rs` module doc: "the narrowest reading"); no spec rule fixes
  them, so only the `create` test carries a `Covers:` claim (§3).

## Mutation counts (`cargo-mutants` 27.1.0)

Command: `cargo mutants -p d2-sim --file crates/d2-sim/src/monsters/init/seams.rs
--file crates/d2-sim/src/wiring/worldgen/init_units.rs --timeout 60 -j 2 -- --lib`.

| File | Before: mutants / caught / unviable / **missed** | After: mutants / caught / unviable / **missed** |
|---|---|---|
| `monsters/init/seams.rs` | 25 / 3 / 5 / **17** | 25 / 19 / 5 / **1** |
| `wiring/worldgen/init_units.rs` | 37 / 23 / 1 / **13** | 39 / 34 / 1 / **4** |
| total | 62 / 26 / 6 / **30** | 64 / 53 / 6 / **5** |

The two extra mutants after are `montype_is` → true / false (caught).

Remaining 5:
- `seams.rs:161` `minions` → `vec![]`: **equivalent** (the default is
  `Vec::new()`).
- `init_units.rs` `boss_quest_hook`, `boss_owner_data` → `()`: they
  forward to `WorldPending` methods whose test double does nothing
  (`TestPending` logs only alignment and presets, and population tests
  assert that log exactly). Observable once a quests / owner-data
  provider exists, or with a logging double.
- `init_units.rs` `after_type_init` → `()`: `prepare_animation` on the
  fixture's monster (no anim data) leaves nothing a test reads.
- `init_units.rs` `set_state` → `()`: no wired caller in the fixture
  (§20 step 5's Countess state) and the fixture state table is empty.

Per-file counts are from each run's `outcomes.json`; the 17 `seams.rs`
survivors before are exactly the list in `mutants-monsters-missiles.md`.
No timeouts in either run.

## Open questions

- **IH1** (spec, `init.md` §17.3): `0x005A0070` ("MonType is that type
  or nested in it") is not listed among the montype matrix readers in
  `runtime-maps.md` §2 (which names `0x00629B50`). The provider reads the
  matrix; confirm in Ghidra that `0x005A0070` reads the matrix (or walks
  the same links: the answers differ only on the walk's depth limit and
  bad links). `TODO(spec: init.md §17.3)` in `init_units.rs`. Local,
  group B (Ghidra only).
- **IH2** (wiring): `has_inventory` / `has_item_at` / `new_inventory`
  wait for a monster inventory provider (items / vendors group); `place`
  / `allocate` / the other creation seams wait for a caller of
  `init::create` in the wiring.

## Local game-file checks

The two edited `#[ignore]` tests only gained a field; they compile in CI
(`cargo clippy --workspace --all-targets`). Umod eligibility (§17.3 r2)
on live tables now follows montype nesting instead of equality, so a
boss whose MonType is nested in a umod's `exclude1`/`exclude2` loses
that umod. Queue: `cargo test -p d2-server --test game_wired_host --
--ignored` and `--test game_world_data -- --ignored` must still pass;
a failure that names a boss's umods is this change (compare with the
original's 0xAC umod lists before touching the expectation).

## Gate

`sh tools/gate.sh`: every step passes. The first run failed only clippy
workspace, test d2-client and doc-tests d2-client, because `wayland-sys`
could not build without the system libraries. After
`sh tools/cloud-setup.sh` those three steps were re-run and pass: clippy
`--workspace --all-targets -D warnings` is clean, and d2-client has 362
passed, 16 skipped. Test counts: d2-sim + conformance 1883 passed, 107
skipped; rest (no client) 599 passed, 75 skipped. Coverage check has 0
errors.
