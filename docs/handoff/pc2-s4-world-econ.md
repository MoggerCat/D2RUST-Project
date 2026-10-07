# PC 2 session 4 — lane world-econ (NPC, vendors, cube, waypoints)

Branch `claude/spec-world-econ-s4`. Every `TODO(spec …)` in `crates/` that
names `world/vendors`, `world/npc`, `world/cube` or `world/waypoints` was
checked against the binary; each is now answered in the spec (below). Many
were already answered by earlier passes (handoff `impl-vendors` V1–V14,
`impl-world` C1–C5, W1); those are listed so implementers can remove the
TODOs (several code readings differ from the answer: marked **code
differs**).

## Written (spec § → behaviour, addresses)

### vendors.md (+ new `vendors-2.md`)
- §7.3 / §7.3.1 (item copy `0x0055A2A0`) moved to `world/vendors-2.md`,
  numbers unchanged, pointer stub left (size; §7.3 has no `Covers:` lines).
- §1 rule 6: record lookup `0x00535F10`; behaviour of every vendor
  function with no record (trade open nothing, no level cap, not
  permanent, on-buy hook / sell fault). TODO `world.rs:505` (design point 1).
- §3.1 step 2, §5.1 step 7: `0x00559CE0` arguments (no-sockets 0,
  never-ethereal 1, no seeds; `0x005764D6`, `0x005789F9`). TODO
  `vendor_world.rs:210`.
- §7 preamble: message order = call order, 0x2A last (TODO
  `wired.rs:730`); item mode setter `0x00624690` (TODO `vendor_world.rs:292`).
- §7.2 rule 3: `0x00557FF0` body (TODO `vendor_inv.rs:262`); rule 9 the
  stored-item removal (stored page, 0x9D action 5 flags 0x20, `0x0055DF10`)
  (TODO `vendor_inv.rs:314`).
- §8.2: `0x005761C0` arguments, which callers pass a player (TODO
  `npc_vendors.rs:135`: none for the NPC services, confirmed).
- §9.2 "Stat readers": totals / base / max durability, (A)(B)(C) only from
  an extended list's full array (TODO `vendor_world.rs:312`; **code
  differs**: a non-extended list gives no (B) terms, not the base array).
- OQ5 decided (fatal → end the game, no message).
- Already answered, code differs: §5.1 `rin`/`amu` missing → item 0
  (`gamble.rs:117`); §3.1 rule 1 upgrade code missing → class 0 → null
  (`store.rs:87`); §3.1 rule 2 null creation → fatal (`store.rs:167`);
  §9.4 missing normal code → fault/fatal (`price.rs:546`); §7.1 rule 2
  GUID = requested (`trade.rs:223`); §7.2 rule 7 mask 4 (`vendors.rs:474`).
  Already answered, code agrees: `gamble.rs:77,103`, `price.rs:174,188,217,443`,
  `store.rs:232,280`, `trade.rs:480,559`; edge case 10 (`wired.rs:743`) is a
  host-clock wiring note, spec unchanged.

### npc.md
- §2 rule 2: AI-think cancel/schedule args (0, 0) (`npc_world.rs:175`).
- §2 start: result dropped by `0x00573020`, 0x13 result 0 (`npc.rs:639`);
  null interaction block faults, never null (`npc.rs:655`).
- §3: 0x2F/0x30 look up the monster list only (`npc_world.rs:117`).
- §5 step 5: pet life signed compare (`npc.rs:900`).
- §7.3: step order with no record, missing Normal row, player level =
  uncapped stat 12 inside `0x006637F0` (`hire.rs:290,300,336`).
- §7.5: missing difficulty row → fatal 0xFA2; slot walk; no slot → refill
  only (`hire.rs:452`).
- §8.1 Socket failure order, leaked duplicate (`services.rs:202`).
- §8.2: addresses were swapped: skill reset `0x00570360` then stat reset
  `0x00570C80`, sound 2 (`npc_world.rs:352`; vitals.md was right).
- §8.3: act change → act completion → waypoint; `0x0054B830` dispatch
  (`services.rs:276`; **code differs**: it runs act completion first).
- `quest_npc.rs:24` (chain 37 `0x0058F870`): answered in
  `quests-act1-rest.md` §9 item 6 (not this lane).

### cube.md
- Summary: item-creation owners. §7.3: `0x005C1BC0` = rare-name pick by
  format, both picks always run (`cube_items.rs:49`). §7.6 step 3: the
  expansion argument of `0x00660240` is never read (`cube_items.rs:357`).
  §8 step 1: `0x0055DF10` failure = fatal (`cube_world.rs:423`). Links to
  `items/generation.md` §12 (repair / recharge / runeword removal, items
  lane) and quest hooks `quests-act2.md` §4.9 / `quests-act3.md` §4.8.
- Already answered: `cube.rs:889` (C1), `:972` (C5), `:1078`, `:1112`
  (C2, C3), `:1178` (C4).

### waypoints.md
- §5.2: 0x13 result of the object case: 0 for every waypoint operate
  (`world.rs:577`, `objects.rs:484`). §7 rule 7: `0x00619E50` is the same
  `0x0066B2B0` call again (`waypoints.rs:152`). `waypoints.rs:513` already
  answered (W1: no room = act 0; **code differs**).

## Pending
- None new. Recording-only questions stay open (npc OQ6, vendors OQ4/6/7,
  cube OQ1–4, waypoints OQ1–3); npc OQ3 (+0x24–+0x26 readers) not settled
  (no blocking behaviour).

## CODE-TABLE CHANGE commits
- None (no TSV changed).

## Cross-file requests
- `world/quests.md` §8.1: "Called … before the act change" is wrong in
  1.14d: `0x00579D60` calls the act change `0x0054B830` first
  (`0x0057A67A`), then `0x005467E0` (`0x0057A688`), then the waypoint
  (`0x0057A696`); change to "after the act change call".
- `items/inventory.md` §1.4 / `0x0055C5C0` ("weapon bookkeeping") has no
  body; `items/generation.md` §12.1 repair calls it last.
