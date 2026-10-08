# q-assassin: Assassin skills in `play` (`claude/q-assassin`)

Nothing here is verified against 1.14d (rule 10). Provisional note: REC-156. Synthetic fixtures only; sound not wired.

## Diagnosis of the 3 failing tests
All three were test-fixture faults; no production code changed:
1. Sentry / Shadow Warrior rows had `pettype` 0 (no pet type), so `pets::add` refused silently (no 0x7A, no error). Fixed with `pettype = 2` and `BodyTables { pettype_count: 3, .. }` as in `app_necro.rs`.
2. Tiger Strike: `skills.stat_count` was 0 in the synthetic table, so `stat_ok(aurastat1)` failed and `charge_hit` returned 0. Fixed with `stat_count = 359`.
3. The rows also lacked the `0xFFFF` sentinels (`aurastate`, `auratargetstate`, `srvoverlay`, `tgtoverlay`, `passivestate`, `delay`, `perdelay`), the `BodyTables` stat/state vectors, and used attack mode 7; they now follow `app_necro.rs` (cast mode 10, `SOSC`).

## Tests (`crates/d2-client/tests/app_assassin.rs`, 5, all pass)
Sentry pet (0x7A), Shadow Warrior pet (0x7A), Tiger Strike charge state, Burst of Speed generic state buff (srvdo 18), a finisher after a charge (asserts the charge and no faults only).

## Left
Dragon Talon / Dragon Flight (`path_op` no-op), Fire Blast / Lightning Sentry missiles, finisher damage and charge consumption, shadow stats, sentry AI, claw weapon check (`itypea1`). Reuse of q-amazon weapon facts / passive refresh and q-summons pets is by the existing wiring; no new copy added.

## The user's local check
```powershell
git fetch origin claude/q-assassin; git checkout claude/q-assassin
cargo test -p d2-client --test app_assassin
```
Expect 5 passed.
