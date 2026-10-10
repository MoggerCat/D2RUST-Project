# rc-missile-class-off hand-back

Cause: the monster family-bolt / DoomKnightMissile missile class is
`srvmissilea` + the monster's chain position (monstats +0x4B, `0x006510C0`;
`specs/skills/bodies-3.md` 4.9, `bodies-4.md` 3.25). The 4 gen-mon rows were
`24/22, 326/323, 78/77, 80/77` (position read as 0).

Result: already fixed on staging by 569af56cf (rc-pets, "chain position").
No code change in this session.

Checks (suite.py, gen checks, orig-cache reused or re-recorded):
| check | class | before (rc-gen-mon-causes) | now |
|---|---|---|---|
| gen-mon-60 fallenshaman3 | 24/22 | DIVERGED@44 | state 150/150, rng 144/144 |
| gen-mon-102, -104 unraveler2/4 | 78/77, 80/77 | DIVERGED | state 150/150, rng 133/133 |
| gen-mon-311, -312, -702, -703, -714 doomknight2/3, dkmag1-3 | 326/323 | DIVERGED | state 150/150, rng 133/133 |
Also equal: gen-mon-646..648, 668..671 (fallenshaman6-8, unraveler6-9).
The state verdict prints PARTIAL with 100% ticks equal (as for other equal gen-mon rows).

Ledger: docs/handoff/ledger/rc-missile-class-off.tsv (8 rows, EQUAL).

Open, not mine: gen-mon-132..135 and -696 (vampire1-6) DIVERGED at frame
45/125, field hp 0 vs 35328..43264 (and rng site 0x5f4e91 missing in
133/135/696). Different cause (vampire hp/transform), 5 checks.
The "326/323" row's example check was not named in the triage file; it was
identified as the doomknight family from bodies-4.md 3.25.
