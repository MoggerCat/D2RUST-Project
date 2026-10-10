# rc-mon-pos2 hand-back

Task: gen-mon cause "monster position 2 subtiles off within its first steps"
(14 checks, example gen-mon-492 frame 38 x 5146 vs d2rs 5148), measured by
rc-gen-mon-triage on r16+.

## Result: no divergence left on the base (integ-r19 merged into staging-7)
- gen-mon-492 on this base: state 150/150 frames equal (0 differences,
  verdict PARTIAL = unmeasured fields only), rng MATCH 133/133.
  Run: `suite.py --checks-dir traces/checks/gen --filter gen-mon-492`.
- The cause list names only the example; the 14 members are not listed, and
  no d2rs code change was needed, so none was made. Presumably fixed by a
  merged r17-r19 change (not bisected).
- Full gen-mon run (335 checks): MATCH 280 / DIVERGED 135 / PARTIAL 255 over
  both channels (state+rng rows); wall 3788 s. No regression: nothing changed.

## Open (not this cause; monster x only, later frames, offset 4 not 2)
- gen-mon-69/70/71/72 (classes 191-194, frame 85/85/85/135), gen-mon-679/716
  (class 681, frame 135): monster 1:9 field x 1.14d 5151 vs d2rs 5147.
  Different shape (4 off, a second monster, late): needs its own triage.
- Other gen-mon state divergences are hp, player m/sp, missile extras, game
  seed (see /tmp run log; owners per rc-gen-mon-causes.tsv).

## No 1.14d reading was needed; no spec/re changes; no ledger rows added.
