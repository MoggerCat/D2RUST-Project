# rc-net-div hand-back (net.s2c / net.c2s divergences; 4 causes, merged to integ-r23)

Claims C022, C023, C024 (docs/handoff/claims.tsv); C014 earlier. Ledger part `ledger/rc-net-div.tsv`.

## Fixed (checks from the orig-cache, fresh d2rs runs)
- C014 (pos poke / store recharge): `pos` poke no longer sends the d2rs-own S->C 0x15 (integ's poke.rs kept);
  store item recharge `0x0055FDE0` sets stat-204 charges to max (integ's `VendorDesk::recharge` kept).
  items-vendor-akara-buy, interact-talk-akara, all 13 a2-npc-* packets MATCH.
- C022/C023 (send order, `a2-quest-tombs` f32, `a2-wp-46` f4): the quest level-change event runs before the room
  switch, but its messages sat in the quest rest buffer, collected after the sim outbox. `QuestLoan::changed_level`
  now drains `rest` into the outbox (as `town_leave`). a2-super-{beetleburst,coldworm,darkelder,fireeye,leatherarm},
  a2-wp-{42,46} packets MATCH. A blanket `HostQuests::send` -> outbox broke the join 0x59/0x5E order: don't.
- C024 (`a2-super-fangskin` f5): the Tainted Sun darken never sent its 0x53/0x5D because `client_in_act` and
  `has_act2` were stubs (false); the act-load hook `0x0059AC40` was never called; the env report's item refresh
  `0x0055FDE0` (= owner refresh `0x00621000(p,1)`) was empty, so 0x47/0x48 after the tick's 0x53 were missing.
  fangskin first divergence f5 -> f46. Specs: quests-act2.md §5.3, quests-act2-2.md §5.3, tick.md §3 step 1.

## EQUAL
+3 net rows (net.c2s.0x32, net.s2c.0x18, 0x27), +3 monster.superunique (beetleburst, dark-elder, fire-eye), +10 rows
(inv.item-use, vendor.{akara,alkor,jamella,malah,ormus,prices,store-gen}, npc.akara,
system.seams.messages.warp-0x07-frame): all REC-2055/2056 (packets MATCH, state PARTIAL only for RunGaps).

## Open a2 first divergences (not mine)
- a2-quest-tombs f35: 0x68 vs 0xA8 (SetState; rc-a8-setstate). a2-quest-arcane f110 / -radament f131: c2s 0x30 missing in d2rs (S).
- a2-quest-summoner f20, a2-wp-74 f14: s2c 0x51 bytes[2] (1.14d 0x57 vs 0x51: object class/shrine? S-M).
- a2-super-fangskin f46: 1.14d 0x4C UnitSkillOnUnit vs d2rs 0xA7 DelayedState (monster skill message order, M).
- a2-quest-taintedsun f52: 0x9C vs 0x07 (item vs reveal order, S-M). items-vendor-drognan/halbu-stock f66: 0x9C vs 0x67; drehya f15: 0x07 vs 0x5D.
- Join-time Tainted Sun (act-load hook at the session join, `d2-server/src/adapters/session.rs`) not wired: only the act-change path.
- Not done: net.s2c 0x0C/0x0D/0x1A/0x6C/0x75 (combat), hire 0x7A/0x42/0x7F/0x9F, net-s2c-chat 0x26, gold-pickup 0x0A.

## Notes
- checks-status.md is overwritten by merges; re-run a check and use the row updater after the merge, not before.
- Disk: `target/debug` + release builds fill the 4G allowance; `cargo clean --release -p d2-client ...` before a build.
