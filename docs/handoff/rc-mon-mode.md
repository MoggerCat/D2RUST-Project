# rc-mon-mode hand-back

Cause fixed: the real host's monster skill check (`0x005FD470`, ai.md §7.4) was
the `Pending` stub (always false), so ClawViper-style sequence skills never
started (1.14d mode 14, d2rs mode 1). Now `View::skill_check`
(`wiring/action/ai.rs`) runs `skill_check` on the rooms (rules 1,3,7,8,9 and the
point tests 2,6; rules 4/5 free point and DiabPrison still answer false).
Second fix: `set_used_skill` in `d2-client/src/app/skill_rest.rs` was a no-op, so
Charge's start never cleared a monster's used skill (infinite mode-start loop).

First divergence, before -> after (EQUAL counts unchanged: no check turned EQUAL):
- gen-su-12: f52 (m) -> f54 (Charge hit/move: monster x/y, player m 19 vs 5)
- gen-mon-77: f31 -> f34; gen-mon-597: f124 -> f127 (same Charge-hit layer)
- gen-mon-298/300/675 (VileMother): f31, unchanged (1.14d mode 14 vs 1)
- gen-mon-206 (CrowNest): f141, unchanged (m 14 vs 1; rng MATCH)

Open:
- Charge hit/knockback layer (player m 19 vs 5, rng site 0x57b0fd / 0x57b5f7 missing). M.
- VileMother `Nest` (srvdofunc 91) still stays neutral: not reaching a sequence
  start; read the VileMother body 0x005F... and the skill start. M.
- CrowNest 206: mode 14 not entered. S-M.
- Not examined: gen-boss-570 f31, champion rows, sys-tick-idle rows, other player `m` rows.
- Spec text for §7.4 wiring and ledger part not written.
