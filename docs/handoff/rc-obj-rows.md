# rc-obj-rows hand-back (2026-10-10)

Task: gen-obj DIVERGED rows (object.init.functions, operate 14/19/20/24/26/3/32/34/39/4/...).

## Result
No code change. The listed DIVERGED rows were stale: they had already been
fixed upstream (rc-object-operate, q-fix-seed-order) and the base ledger still
carried the old verdicts.

Re-run of all 571 `gen-obj-*` checks (orig-cache reused for 19; the rest
re-recorded under Wine; 6,696 s wall):
- state: 565 PARTIAL (equal where compared; fields one side does not write),
  6 DIVERGED
- items: 84 MATCH, 487 PARTIAL, 0 DIVERGED
- rng: 569 MATCH, 2 DIVERGED
- Ledger: 16 operate rows (corpse 61, armorstand 4, weaponrack 4, urn 25, chest 74,
  bank, portal, cube/scroll/staff chests, khalim chests 1-3, stair 2, ...)
  now equal where compared (0 diverging checks each). EQUAL count in the
  ledger is unchanged at 883 (these rows settle at PARTIAL/NO-CHECK, as
  rc-object-operate's do); DIVERGED rows 1061 -> 1045.
- Ledger part: docs/handoff/ledger/rc-obj-rows.tsv (17 rows, base regenerated).

## Open (6 checks, each a different cause, all OperateFn 0 objects)
- gen-obj-36 (Dummy Standard2) and gen-obj-39 (RogueBonfire): frame 20 player ty
  1.14d 4216 vs d2rs 4218 (36), 4242 vs 4237 (39). 39 also has an extra rng draw
  at frame 44 on the d2rs side. Size S each.
- gen-obj-61 (InitFn 13) and gen-obj-189 (cain portal, InitFn 61): object mode `m`
  differs at the operate frame (61: 1.14d 0 vs d2rs 2; 189: 2 vs 1). Size S.
- gen-obj-78 (invisible town sound): player mode `m` 1.14d 5 vs d2rs 3 at frame 33. Size S.
- gen-obj-369 (trapped soul, InitFn 46): game seed pair differs at frame 10 and
  the original has an rng draw at site 0x552e31 that d2rs lacks. Size S.
`object.init.functions` stays DIVERGED because of 61, 189, 369 (3 of 333
object rows with a nonzero InitFn).
These are separate causes, not one group; I stopped without picking one.
No fidelity code, spec or private-repo file was changed.
