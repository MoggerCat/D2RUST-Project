# rc-mon-ai-draw hand-back (2026-10-10)

Task: "d2rs AI think draws one extra time" (8 gen-mon checks, first divergence
frame 69, rng extra at ai/mod.rs:412 `chance`; rc-gen-mon-causes.tsv row 7).

## Result: already fixed on staging-7 + integ-r23, no code change
Checks (gen-mon-469..473, 634, 635, 719, plus 714/715 in the same filter;
`--no-playthrough`, 1.14d re-recorded under Wine): rng MATCH 146/146 (469),
145/145 (470, 471), 144/144 (472, 473, 719); state 150/150 PARTIAL
(q, seed ignored by the check); all 9 checks MATCH on rng.
No divergence at frame 69 any more.

## Why
The extra draw was the succubus (`AITHINK_Fn118` 0x005E1E00) "target cursed"
test (`0x00625760(T,0x20)`) reading a stub: d2rs saw "not cursed" and drew
P(aip3) a second time. Fixed by rc-mon-s (REC-2240: has_list_flag/max_mana);
the next divergence (frame ~120, missing 0x57DB38 draw) by rc-succubus-m
(REC-2325). I read 0x005E1E00 again against `bodies5.rs::succubus`: step order
and draws agree. The causes table row is stale; it predates those two parts.

## Changed
Nothing in code. This hand-back only.

## Open
- Playthrough (act 5): nihlathak-killed stuck at frame 1000; not this cause.
