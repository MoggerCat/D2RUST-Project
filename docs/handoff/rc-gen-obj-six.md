# rc-gen-obj-six hand-back (2026-10-10)

Task: the six gen-obj DIVERGED state rows left after rc-obj-rows (36, 39, 61, 78, 189, 369).
Base: specs-staging-7 + integ-r19. Full 571-check re-run not finished (coordinator runs one).

## Checks (state / rng, single runs, orig-cache)
- gen-obj-36: already equal after the r19 merge (PARTIAL / MATCH). No change.
- gen-obj-61: DIVERGED -> PARTIAL, rng MATCH.
- gen-obj-189: DIVERGED -> PARTIAL, rng MATCH.
- gen-obj-369: DIVERGED (state + rng) -> PARTIAL, rng MATCH.
- gen-obj-39, gen-obj-78: still DIVERGED (open below).
EQUAL rows: 4 of 6 now clean (36, 61, 189, 369).

## What changed (one commit per cause)
- REC-2330 init 13 (0x00594020): quest-routed; chain 4 record present -> link (mode stays 0), else mode 2.
- REC-2331 d2-client sync snapshot now holds objects, so quest object event 7 sees the cain portal's act (it fell to the non-Act-I branch, event never moved mode 1 -> 2).
- REC-2332 init 46 (0x005506D0) trapped soul placeholder implemented (world/quests/placeholders.rs, spec objects-2.md section 17), 2 unit tests.

## Open (size S-M, one cause, path code)
gen-obj-39 (fire, size 3) and gen-obj-78 (sound dummy, size 0): first run path matches 1.14d,
the player stops (blocked step / arrival), then 1.14d's second path compute returns 0 and the
player goes neutral (mode 5); d2rs's compute (type 7, target unit) returns a 3-4 point detour via
A*/target preparation (39: target (4881,4242); 78: points (4869,4234)...(4859,4243)). Suspects:
the object footprint stamped for the target (fire sideways clearance) or A* limits in 1.14d.
Needs a 1.14d collision/path dump near (4877,4242) and (4868,4233). 39 also has one extra rng draw
at frame 44, a consequence of the same run. Not fixed.
