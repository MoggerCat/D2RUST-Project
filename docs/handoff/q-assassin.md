# q-assassin: Assassin skills in `play` (`claude/q-assassin`) — UNFINISHED

Stopped early by the coordinator. Nothing here is verified against 1.14d (rule 10). No REC id was taken (REC-156 was reserved for any provisional note; none written yet). Synthetic fixtures only.

## State

- **Tests written, not yet compiled or run**: `crates/d2-client/tests/app_assassin.rs` (3 tests, harness copied from `app_amazon.rs`, with a synthetic states table whose rows 8 and 9 are progressive):
  - `a_sentry_trap_is_laid_and_listed_as_a_pet` (srvdo 45, expects S→C 0x7A)
  - `shadow_warrior_summons_the_shadow` (srvdo 49, expects 0x7A)
  - `tiger_strike_adds_a_charge_on_a_hit` (srvst 23 / srvdo 34, expects the pgsv state on the player)
- The first `cargo build` failed because the system libs (wayland etc.) were missing; they are now installed, but the test build had not finished. Expect compile fixes in the test file (field names/types of `Skills`, `StatLists::has_state`, `BodyTables` pub fields) before any result.
- No source change in `crates/` yet; no missing link has been confirmed.

## Findings from reading (unverified)

- The sim already has bodies for: charge-ups `srvst 23` / `srvdo 34, 35` and the progressive functions 36–39, 40, 41, 143 (`bodies.md` §2.14, §8.8, §8.10, `bodies-4.md` §4); sentries `srvdo 45` (§8.3, `helpers2::sentry`); Shadow Warrior `srvdo 49` (§8.21); Dragon Talon `srvst 24` / `srvdo 42`; Dragon Claw 25/46; Blade Sentinel 44; Cloak of Shadows 47; Blade Fury 48; Dragon Tail 50; Mind Blast 51; Dragon Flight 52.
- Charge-ups need `states` rows (`state_count`), `aurastate` below the count, and `hooks.bodies` (pet types) for sentries/shadows; the synthetic game has none, the new test installs them.
- Likely gaps to check first: `Pending::body_effect` for `Alignment`, `OwnerData`, `NodeInsert` (pets' real AI, see q-summons "Left"); `path_op` (Dragon Talon kick / Dragon Flight move) is the default no-op; `line_clear` still blocked (skills with `lineofsight` > 0).
- Burst of Speed and Fire Blast were not traced (no special srvdo found in `functions.tsv`; probably the generic state buff / missile paths).

## What's left

1. Compile and run `app_assassin.rs`; fix links that fail (each test must fail before the fix).
2. Dragon Talon (kick, `path_op`), Burst of Speed (state + speed stat), Fire Blast.
3. Tests for the finisher (second skill after charges) and the shadow's stats.
4. REC id: take the highest REC-N in `origin/claude/specs-staging-7` docs/HANDOFF.md §7 plus one; add PROVISIONAL notes.
5. `cargo fmt`, clippy, `cargo nextest run -p d2-sim -p d2-server -p d2-client -p test-fixtures`, `tools/coverage.py --check`, `tools/spec_index.py --check`.

## The user's local check

```powershell
git fetch origin claude/q-assassin; git checkout claude/q-assassin
cargo test -p d2-client --test app_assassin
```
Report compile errors or failing test names.
