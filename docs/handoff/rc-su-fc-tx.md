# rc-su-fc-tx (REC-2790)

gen-su-* (66 checks, state channel, pokes only): EQUAL (state PARTIAL 150/150, only
the d2rs client gap, REC-2055) 47 -> 48 at the fresh run; ledger part
`ledger/rc-su-fc-tx.tsv` settles 54 superunique rows (38 EQUAL, 16 DIVERGED with fresh
first divergences). Before this session's fixes the fresh run was already 47: the
"fc" rows (gen-su-11/25/49/50/61) and most "tx" rows (47/8/22/36/51/52/64) were fixed upstream.

## Changed
1. Hydra summon alignment (`0x005543B0(m, alignment(owner), 1)`, skills/bodies-3.md §8.5):
   the `Alignment` body effect was not wired, so a Council member's Hydras kept the
   monstats class alignment (good) and targeted the Council member; now they take the
   owner's (evil) and aim the Hydra missile at the player (gen-su-26..31 tx 5153 -> 5143).
   `wiring/interaction/skill_use.rs`.
2. Knockback request ignored when the *requested unit* is in mode 19 (`0x0057F190` reads
   its own unit +0x10, not the target's): gen-su-65 tx 5140 -> 5143. `path/walk/request.rs`,
   spec pathing.md §1.2 r5, mutant test corrected.

## Open
- gen-su-26..31: first divergence now player `m` 4 vs 0 (gen-su-29: seed) after the Hydra
  missile hit (hit-reaction / will-die flag on the player), size M, not investigated.
- Other DIVERGED gen-su rows are seed / monster mode `m` (rc-su-mode) causes.
- gen-mon regression sample not run: no gen-mon 1.14d recordings cached here (fix is
  limited to Council Hydras and a knockback double-request).
