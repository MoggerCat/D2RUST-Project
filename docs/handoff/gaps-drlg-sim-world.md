# Gap tests: drlg, sim (tick, units, unit order, intents, stats, stat lists), world

> Not yet folded into `docs/HANDOFF.md` (§1, §3, §7) and `docs/PLAN.md`; a docs session folds it, then this file stays as the detailed record.

Cloud implementation session, 2026-10-06, task class: unit tests from
clear specs, medium (METHODS M14). Branch `claude/gaps-drlg-sim-world`,
based on main at `edad871`. Repo only, synthetic data, no game files
(M09). Unit-tier claims only, so no rule became "verified" (CLAUDE.md
rule 10, M02). Each claim names only what its test asserts (HANDOFF §8
lesson on overclaiming).

## 1. State

Scope was every unclaimed unit of `specs/drlg/*.md`,
`specs/sim/{tick,units,unit-order,intents-events,stats,stat-lists}.md`
and `specs/world/*.md` (99 units at `edad871`). The earlier gap sessions
(`gaps-drlg-world.md`, `gaps-items-stats.md`) already gave a reason for
nearly all of them; no spec in scope changed since. This session re-read
each remaining unit against the spec and the code and found three units
whose outcome a test can check in full; the rest keep their recorded
reasons (§5).

| Spec | Before (any tier) | After |
|---|---|---|
| specs/drlg/outdoor.md | 64/72 | 65/72 |
| specs/sim/unit-order.md | 36/38 | 38/38 |
| repository total (any) | 2401/2704 (88.8%) | 2404/2704 (88.9%) |

(`python3 tools/coverage.py --summary` on this branch; verified stays
216.)

## 2. Code map

| File | What |
|---|---|
| `crates/d2-sim/src/drlg/outdoor/linker_tests.rs` (new) | `linkers_draw_retry_and_place`: every linker of `outdoor.md` §2.4 called directly (`run_linker`, private, so the file is a child module of `place.rs`): first-call draws on the driver's seed copy (none for Def / Fix, one for R4 / R8 / VS / RW / B2 / OS / B1 / BD, two for BM / RE), each retry's alternative in order, the `false` that ends R4 / R8 / VS / BM / RE / RW / B2, retries that always succeed (Def, Fix) or draw again (OS, B1, BD), the BM size switch, the OS transition flag, the B1 / BD / B2 rects and the B2 offset index `R0 + 2·R0[link]` for both link values. Seeds 1..64 reach every first value |
| `crates/d2-sim/src/drlg/outdoor/place.rs` | +3 lines: `#[cfg(test)] #[path = "linker_tests.rs"] mod linker_tests;` (no code change) |
| `crates/d2-sim/src/units/gap_tests.rs` | `guid_drawn_once_after_seed_before_init` (`unit-order.md` §1 r4, r5): one counter step per allocation, the seed drawn first and the GUID already set when the per-type init runs; a fixed GUID (monster flag 2) leaves the counter where it was while the seed is still derived; a player draws from counter 0 only, with no seed draw. The `Probe` fake gained `init_guids` (record GUID and the type's counter seen by `init_kind`) |

M08 by hand: BM / RE retry `r := (R2 + R0 + 1) mod 4` and VS placing
with variant 1 each failed `linkers_draw_retry_and_place`.

## 3. Signature changes

None. No public item changed.

## 4. Seams reached

None new. `outdoor.md` §2.4's B2 index outside 0..3 (R0[link] not 0
or 1) stays the existing `TODO(outdoor.md §2.4)` in `place.rs`; the test
only uses R0[link] ∈ {0, 1}, as 1.14d's tables do (the link is the B1
row).

## 5. Units left unclaimed

All keep the reasons recorded in `gaps-drlg-world.md` ("Not claimed,
with reasons") and `gaps-items-stats.md` (unclaimed table); re-checked
here against the specs at `edad871`. Additions:

- `outdoor.md` §2.4: now claimed (was "partly tested").
- `unit-order.md` §1 r4, r5: now claimed (the missing halves of
  `units/tests.rs::allocation_seeds_and_rejections`).
- `tick.md` §5.3: the section holds the every-tick mechanics and the
  table of its three callers; the druid skill caller (`0x005D18E0`) is
  behind the per-skill function seam, so a claim on the section would
  overstate. `tick.md` §5.2 r4: `needs_uninterruptable_check` exists
  but no caller runs `0x005544B0` (monster spec).
- `intents-events.md` §3.2 r3, r4: `Host::flush` implements the 40 ms
  gate and the per-client pop order, but not the empty-game timeout
  (r3) or the failed-send count and drop (r4); not claimed.
- `outdoor-tilesub.md` §2.1: the per-act context table (callbacks, T,
  B per caller) is built by the act code; only part is reachable from
  `tilesub.rs`.
- `maze.md` edge r9, `rooms.md` edge r3, r4, `levels.md` edge r3:
  unreachable rows, stack overflow and address order (open questions).

## 6. Questions

None new. The open ones stay: `gaps-drlg-world.md` 1–7 and
`gaps-items-stats.md` 3–9 (HANDOFF §7 GI / GD / GW), and the known
deviation `units.md §5 r4` vs `TimerQueue::schedule` (HANDOFF §1 3m,
§7 GI), left unfixed: `tick.md` §5.2 still does not say the callback is
dropped.

## 7. Local checks to queue

None: unit tier only, nothing reads game files.

## 8. Gate results

Run on this branch before the push:

- `cargo fmt --check`: pass
- `cargo clippy --workspace --all-targets -- -D warnings` (after
  `sh tools/cloud-setup.sh`): pass
- `cargo test -p d2-sim`: 1208 passed, 0 failed, 5 ignored (+1 ignored doc test); no other crate touched
- `cargo run -p depcheck`: pass
- `python3 tools/spec_index.py --check`, `python3 tools/methods.py
  check`, `python3 tools/coverage.py --check` and `--selftest`: pass

Non-source files in the diff: this note only.
