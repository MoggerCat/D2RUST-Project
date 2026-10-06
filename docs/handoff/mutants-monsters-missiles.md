# Mutation testing: monsters, missiles

> To be folded into `docs/HANDOFF.md` (§1–§5, §7) and `docs/PLAN.md` by a docs session; this file stays as the detailed record.

Branch `claude/mutants-monsters-missiles`, from `claude/tender-meitner-mphas3`
at `4b5b0bf`. Cloud test session, medium effort, METHODS M08 (prove the
check can fail) applied to `crates/d2-sim/src/monsters/**` and
`crates/d2-sim/src/missiles/**`. Only test code changed: no non-test
code, no spec, no `docs/HANDOFF.md` or `docs/PLAN.md`. Every new test
is unit tier, so nothing became "verified" (CLAUDE.md rule 10).

## Counts (`cargo-mutants` 27.1.0)

Command: `cargo mutants -p d2-sim --file 'crates/d2-sim/src/missiles/**'
--file 'crates/d2-sim/src/monsters/**' --timeout 10 -j 3 -- --lib`.
The "after" run is the same command with `--iterate --timeout 20` on the
"before" output, so it re-tests only the missed and timed-out mutants.
`mutants.out` is not committed.

| Area | Mutants | Before: caught / timeout / unviable / **missed** | After: caught / timeout / unviable / **missed** |
|---|---|---|---|
| missiles | 362 | 275 / 23 / 10 / **54** | 323 / 23 / 10 / **6** |
| monsters | 2099 | 1783 / 7 / 41 / **268** | 2005 / 9 / 41 / **44** |
| total | 2461 | 2058 / 30 / 51 / **322** | 2328 / 32 / 51 / **50** |

A timeout counts as detected: the mutant made a test loop or hang. The
after run reported 51 missed. One more, `ai/mod.rs:620:42`, was then
killed by `install_switches_to_alternate_only_when_nonzero`; that kill
was checked by applying the mutation by hand. The two new timeouts are
`population/region.rs:383:27` (`-=` → `+=`, `/=`), whose test hangs.

## Tests

121 new tests, 115 with `// Covers:` claims, each on the rule whose
outcome it asserts. Files are new, apart from the `mod` lines. A file
that needs a module's private fakes is a child of that module's test
module through `#[path]`:

| File | Tests | Mounted from |
|---|---|---|
| `missiles/mutant_tests.rs` | 21 | `missiles/tests.rs` |
| `monsters/mutant_tests.rs` | 10 | `monsters/mod.rs` (public APIs only) |
| `monsters/mutant_tests/ai.rs` | 28 | `monsters/ai/tests/rules.rs` |
| `monsters/mutant_tests/init.rs` | 7 | `monsters/init/tests.rs` |
| `monsters/mutant_tests/umods.rs` | 16 | `monsters/init/tests.rs` |
| `monsters/mutant_tests/population.rs` | 39 | `monsters/population/tests/mod.rs` |

Test fakes gained knobs only; existing tests are unchanged and still pass:
- missiles `Fake`: `hirelings` set, and `add_target_ac` logs `ac <unit> <delta>` (§R6.1 r3 was unobservable before);
- AI `Fake`: `acts` per unit and `take_alt`, and `choose_alternative` logs `alt <unit>` (§5.2 slots 8/9);
- init `Fake`: a `register_spawn` override logging `register <unit> <nc>` (population §9.6 r2);
- population `Fake`: `nearest` point for `nearest_free_point` (§6.3 r4 last fallback).

Some kills came from assertions the old tests lacked, not from new
setups. Examples: `unit_fields_mode_flags_and_size` could not see a
zero-valued `BIT1` constant, and the archer E = 6 case matched only the
draw count.

## The 50 remaining survivors

None is a case where the spec decides the outcome and no test checks it.

**Equivalent mutants (same behaviour on every input):**
- `missiles/catalogue.rs:241` and `flight.rs:148`, `|` → `^`: the operands are disjoint bits.
- `hit.rs:15`, `HIT = 1 << 0` → `1 >> 0`: both are 1.
- `flight.rs:45`, velocity `< 0` → `<= 0`: both give 0.
- `hit.rs:65`, swap when min `>=` max: swapping equal values changes nothing.
- `hit.rs:111`, poison count `>= 1`: dividing by 1 changes nothing.
- `ai/mod.rs:576`, base 118 arm deleted: mode 2 is handled inline before the match, so that arm is never reached.
- `init/umods.rs:343`, `|` → `^`: disjoint flags.
- `umods.rs:393`, `412`, weight `>= 0`: a zero weight adds nothing and is never picked.
- `umods.rs:544`, `Gate::None` arm deleted: it falls to the same empty arm.
- `umods.rs:808`, `+ 0` → `- 0`: every aura row has offset 0.
- `population/placement.rs:90`: the `0` arm comes first.
- `placement.rs:126`: `l == rr` holds exactly when `t == b`.
- `placement.rs:151`: the wildcard arm also returns true.
- `placement.rs:188`, `x + dx` → `x - dx`: the offsets are symmetric and the spec gives no order.
- `preset.rs:344`: both paths return None.
- `preset.rs:396`, `never_count_flags` → 0: `create` ORs `neverCount` in anyway.
- `preset.rs:483`: every Never row already returns at `M::None`.
- `region.rs:312`: e ≤ 12 here.
- `region.rs:336`: k is already 3 − v.
- `room.rs:162`: the stored value equals the cap.
- `room.rs:256` (2 mutants): the wanderer table index is always 0.

**Unobservable: unreachable with valid tables, or a d2rs default no spec rule fixes:**
- 17 default bodies of `InitHost` methods in `init/seams.rs` (`level_id`, `region_variant_count`, `region_variant`, `has_inventory`, `has_item_at`, `place` ×9, `minions`, `montype_is`). They are the "no provider" values; every host overrides them.
- `ai/mod.rs:306`, the `base_class` missing-row default: callers compare against specific classes, never ±1.
- `ai/mod.rs:312`, `skill` with no monstats row: `think` returns before any AI function runs.
- `ai/mod.rs:344`: special state 18 cannot be installed (§3.3 r2).
- `init/mod.rs:293` and `init/create.rs:226`: a record shorter than 0x188, or an extra-table shorter than monstats. Neither happens with the fixed-size 424-byte records.
- `ai/functions.rs:282`, shaman `count > 0` → `>= 0`: the corpse scan returns a corpse exactly when its count is above 0 (`shaman_corpses` seam contract).

**Spec gaps (the spec does not decide; no test written):**
- `ai/functions.rs:453`: Wraith (§9.11) with no target T. The code does nothing; the spec only says "walk in radius of T".
- `ai/functions.rs:583`: Navi (§9.10 r2) "clamp param 1 at 0 and count it down" with a negative param 1. The code gives 0; the mutant gives −1. Param 1 is only ever set to 60, so this is unreachable today.
- `population/preset.rs:614`: §11.6 r3 when the class's `BaseId` row is invalid (for example −1). The code returns the input class; the spec does not say.

## Code vs spec

No mutant showed the code disagreeing with its spec, and no non-test code
was changed.

## Gate (this branch)

- `cargo fmt --all --check`
- `sh tools/cloud-setup.sh`, then `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test -p d2-sim`
- `cargo run -p depcheck`
- `python3 tools/spec_index.py --check`
- `python3 tools/methods.py check`
- `python3 tools/coverage.py --check` and `--selftest`

All pass at this commit: fmt clean; clippy clean (whole workspace);
`cargo test -p d2-sim` 1353 passed, 5 ignored (lib) + 1 ignored
(doc/integration); depcheck OK (8 crates, determinism lint clean);
spec index OK; `METHODS.md: 21 methods OK`; coverage 3376 claims, 0
errors; coverage selftest ok.
