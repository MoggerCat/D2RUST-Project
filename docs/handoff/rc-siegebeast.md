# rc-siegebeast hand-back

Checks (state + rng; base = integ-r17 merge):
- gen-mon-441/442/443: DIVERGED -> DIVERGED, first divergence later:
  frame 50 -> 90 (441, 442), 50 -> 82 (443). EQUAL rows 0 -> 0.
- gen-mon-610..612 (slinger7-9): state PARTIAL 100%, rng MATCH (EQUAL).
- gen-boss-526 (nihlathak): DIVERGED frame 69 monster 1:8, field tx
  (1.14d 5147 vs d2rs 5156). Not touched; size S-M.

Causes fixed (d2-sim):
1. Possessed-imp think `0x005E2D80` read the wrong monstats AIP columns:
   `+0x5C` = aip2, `+0x62` = aip3, `+0x68` = aip4 (aip1 = `+0x56`). Teleport
   range = imp1 aip2; act test = dist < imp2 aip2, roll < imp2 aip3; the
   two extra rolls use 2 x imp2 aip4 (`all.asm` 0x5E2EAC). Spec
   `monsters/ai.md` §3.3 corrected (`bodies2.rs`).
2. `link_source` (`0x00621C30`) only filled a side map; the unit record's
   +0x94/+0x98 and +0xC8 bit 0x400 were not written, so
   `SUNIT_GetOwner(beast)` stayed none after possession and the other imps
   kept walking to the beast instead of falling through to their normal
   think (5 draws at frame 50). Now written (`skill_use.rs`).

Open:
- 441-443 frame 90: unit 1:11 (possessing imp) rng site `0x5a55ba` missing;
  state m 4 vs 5 on the player (443: missile lvl at frame 82). Same site as
  rc-mon-tx 692/693. Size M.
- Gates run: fmt, clippy -p d2-sim, nextest -p d2-sim (4768 pass).
