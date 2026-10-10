# rc-mon-frame31: hand-back (2026-10-10, branch `claude/rc-mon-frame31`)

Base: `claude/specs-staging-7`. REC-1995..1998 used.

## Checks (suite, `traces/checks/gen`; EQUAL = every channel MATCH/PARTIAL)

| Set | Before | After |
|---|---|---|
| `gen-su-*,gen-boss-*,gen-umod-*` | 53 / 133 (rc-mon-spawn-think; not re-run here) | 81 / 133 |
| Putrid 546, 547 (gen-mon) | frame 31 (m) | EQUAL |
| VileMother 298, 299 | frame 31 (m) | EQUAL |
| Megademon 362, 686 | frame 31 (m) | frame 52 / 49 (field fr) |
| ClawViper 77 | frame 31 (m) | frame 34 (player m: the Charge hit) |

Only `gen-boss-570` still diverges at frame 31 (class 570, 1.14d mode 0, d2rs 1): not
looked at (S, a death-mode start; first look: its mode-0 start on the spawn frame).

## Causes (four, all seams that answered "nothing")

1. Natural monster skill levels (init step 14) were never kept: the AI's read of
   `Skill1` level (`0x006442A0`) fell back to 1, so Megademon (range = level) never
   entered its in-range branch (REC-1995, `ActionHooks::natural_skills`).
2. `0x005FD470` skill check was the host stub (false): ClawViper never cast
   SerpentCharge. Wired the existing `skill_check` rules to the DRLG rooms (REC-1996).
   Rule 5 (DiabPrison) still false.
3. Monster `0x0063E9F0` boss test was false: Putrid (monstats boss=1) skips the
   precheck-C boss sound (idle 20) (REC-1997).
4. VileMother birth: `0x005FD350` footprint (flag 0) and `0x00621DC0` direction were
   stubs (REC-1998). Also the client's `set_used_skill(None)` was a no-op, which made
   Charge's melee branch recurse (stack overflow) once the cast started.

## Open

- Megademon: next layer at frame 52 / 49 (`fr`, 1.14d 2752 vs 2560) and rng at 62 / 59
  (site `0x5a898e`, a missile/attack draw): S-M.
- ClawViper 77: the SerpentCharge hit on the player (frame 34, 1.14d m 19): M, combat/charge owner.
- `gen-boss-570` frame 31, see above.
- `is_boss` in `combat.rs`/`pending.rs` is still the stub (damage/hit rules read it).
- Ledger: `ledger/rc-mon-frame31.tsv` (7 rows). `cargo nextest -p d2-sim` 4750 pass; d2-client: clippy --all-targets clean, its tests not run (disk).
