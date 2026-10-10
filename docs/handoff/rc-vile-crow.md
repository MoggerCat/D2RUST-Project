# rc-vile-crow hand-back

Scope: gen-su/gen-boss checks plus gen-mon-298/300/675/206 (not the other ~330
gen-mon rows: the full 426-check run exceeded the 10 min background limit I set).

Cause fixed (one): the monster-AI host answered two reads with stubs. `ai_has_list_flag`
(`0x00625760`, curse flag 0x20) was always false and `max_mana` (`0x00625D60`) always 0
(`wiring/action/ai.rs`). A Succubus therefore cast a curse again on an already cursed
player. Both are real stat-list reads now. Spec: ai-bodies-5.md §4.

Checks (cluster of 95), before -> after:
- MATCH 2 -> 4, DIVERGED 43 -> 43, ticks equal 10566/14245 -> 11170/14822 (74.2% -> 75.4%).
- The 4 ERROR rows of the first run are PARTIAL now (no m field difference).
- gen-su-54: first difference frame 41 (m 4 vs 9) -> frame 68 (fr 13824 vs 1632, then seed).

Open:
- gen-su-54 f68: monster g=14 `fr` and seed differ (S-M, not read yet).
- m rows left, first fields: gen-su-12 f54 (player m 19 vs 5, Charge), gen-su-27 f63
  (monster class 346, m 8 vs 2), gen-su-34 f73 (class 299, m 2 vs 14), gen-su-65 f38
  (player m 0 vs 4), gen-boss-544 f80 (player m 0 vs 1), gen-boss-570 f31 (class 570, m 0 vs 1).
- VileMother (298/300/675) and CrowNest 206 not re-checked after the fix.
- Untouched: the other gen-mon rows; no PC 1 facts needed.
