# rc-damage-draws hand-back (2026-10-10)

Branch `claude/rc-damage-draws` (from specs-staging-7 + integ-r19). REC-2355..2358.

## Checks (gen-mon, the 51 DIVERGED rows of `rc-gen-mon-triage.tsv`)
EQUAL (state + rng, harness-ignored fields as before): 0 -> 23 of 51.
Also equal now (were not in the DIVERGED list at r19): gen-mon-190, 475, 618.
Ledger: `docs/handoff/ledger/rc-damage-draws.tsv` (23 rows EQUAL).

## What changed (one commit per cause)
1. **Used skill cleared by the mode request builder** (REC-2355, `0x005A7E60`
   `0x00620210(unit,0)`; skill requests set the skill after it:
   `0x005DEAD0`, `0x005DE000`). d2rs left the Smite/sequence skill set, so the
   next plain A1 ran the skill body (mode damage 5, wrong n). Fixes the
   "damage-roll draw count differs 0x57b5f7" cause (189, 190, 475, 618, ...).
   `ai/mod.rs` `request_mode_byte` clears, `request_mode_keep_skill` for the
   skill callers; seam `clear_current_skill`. Spec: `monsters/ai.md` §7.1.
2. **Missile bodies read max life/mana** (REC-2356): `MissileBodies for View`
   had the default 0, so server-hit 31 (vampire fire head heal) set the owner's
   life to 0 (gen-mon-132, "d2rs monster dies while 1.14d lives").
3. **Suicide explosion line test** (REC-2357): `umod_line_clear` called the
   host default (always blocked); now the collision line test (mask 0x805),
   so the explosion hits the player (466: the `0x57b0fd` cold draw on the player).
4. **Monster melee range + Charge class test** (REC-2358): the combat host used
   a preview Chebyshev range for monsters; now `monster_in_melee_range` (reach
   `MeleeRng`+extra+1, unit distance, line 0x804) for every monster attacker
   (595: Charge hit one frame early). Charge's `mode_damage` mode keys on the
   class (`0x00463900`), not `BaseId` (`0x005CFCA2`); spec `skills/bodies-2.md` §5.4.

## Not touched / open
- "d2rs game-seed draw at unit removal (`lifecycle.rs:180`, 7)" and
  "1.14d game-seed draw at `0x552e31`" (6): no longer in the 51 after the
  above (475, 190 equal); not separately fixed. Cause list not re-triaged.
- 28 of the 51 still diverge, other causes (counts by first difference):
  - monster-fired missile hits the player: d2rs rolls the missile damage
    (`missiles/hit.rs:140`) but does not apply it, 1.14d continues with
    `0x5a55ba` (+ cold `0x57b0fd`): 441-443, 529-533, 645 (9). Missile area.
  - missile unit extra in d2rs (1.14d has none): 279-281, 362, 662, 664, 686,
    687, 712 (9). Missile area.
  - player `sp` 128 vs 64 (anim speed) 295, 576, 578, 624, 629, 630, 652 (7).
  - monster x 5151 vs 5147 at frame 85: 69-71 (3).
- Not run: clippy on `d2-client` (3 lines in `single_player.rs`; disk 3.8 GB
  left; the release build of d2-client compiles). `cargo nextest -p d2-sim`:
  4770 passed. Full gen-mon (335) not re-run (too slow); the 51 above were.
