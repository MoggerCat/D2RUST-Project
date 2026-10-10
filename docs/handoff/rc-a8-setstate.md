# rc-a8-setstate hand-back
Cause 1 (0xA8 byte 9): EQUAL 2759 -> 2801 on its base. Cause 2 (right aura at join): EQUAL 3195 -> 3213 on integ-r23.
Cause 3 (0xA7 skill delay): EQUAL 3270 -> 3294 on integ-r23 (incl. ledger.py --fix of stale rows in 11 parts).
Cause 4 (item stream regression): EQUAL 3328 -> 3353 on integ-r23. gen-skill packets MATCH 91 -> 187 of 210.
## Cause 1: S->C 0xA8 byte 9 at frame 2 (assassin masteries, ~60 checks)
- 1.14d `0x00646D60` sets `passivestat1-5` on layer `passiveitype` (`0x00627150(list, s, v, layer)`), sent as the
  0xA8 entry's param (`0x005711D0`); d2rs used layer 0. Fix: `BodyWorld::list_set_layer` in `passive::refresh`
  (spec `client/msg-skills.md` §2 r4; test `tests6::passive_refresh_sets_the_stats_on_the_passiveitype_layer`).

## Cause 2: paladin right aura at join (causes-99 C018 0xAA size, C026 0xA8 size; PROVISIONAL REC-3410)
- 1.14d (`intents-events.md` §8.2 r3.1, `d2s.md` §2.4 r6.3): the join 0xAA goes out at player creation.
  The post-load select `0x005701B0` -> `0x0056FF10` comes later and starts the aura:
  - non-immediate: state on, list with only 350/351;
  - immediate: the do `0x0056F7F0` applies its stats, already in the first 0x95 (pal-115 Vigor stamina).
  The state reaches the client as 0xA8 at the first update.
- d2rs:
  - the aura starts once at load (it ran twice);
  - `UseView::set_aura_state` makes the markers-only list (the client seam only logged it);
  - the state's unit bit is hidden through the join and turned back on after it, with the passive states (as REC-2105).
- `0x00639E30` queues the unit itself (`0x0064C040`): the skill bodies' `mark_state_changed` now queues too
  (the frame-51 0xA8 of Holy Fire / Freeze / Shock ...). `sim/stat-lists.md` §9.2 corrected.
- gen-skill-pal packets MATCH 9 -> 29/30. The 0xA8 rows' hand-written checks plus pal-* (69): packets MATCH 64.

## Cause 3: S->C 0xA7 state 121 missing at the cast (13 checks: Fire Wall, Blade Sentinel, druid summons ...)
- `0x0056EF90` (set_delay) switches state 121 on with `0x00639DB0`, which always queues the unit; d2rs didn't queue.
  Fixed in `UseView::create_delay_list`; `skills/use.md` §6 says so.

## Cause 4: unidentified socketed item's socket count (gen-item* / items-* / vendor 0x9C byte 37)
- `aa5de032c` (integ-r11) reverted the writer part of `3f2b34259` until `item_bits::decode` could skip the bits.
  The reader part landed, the writer never came back. Re-applied (`git revert aa5de032c`); `prop_item_bits` passes.
- gen-item* 57/57 MATCH; the 7 gen-sysc NPC checks and items-vendor-* MATCH; 14 items-* channels fixed.
  items-drops-nor-09/12 (drop category order) diverge on plain integ-r23 head too: not this change, not bisected.
## Ledger / status
- `ledger/rc-a8-setstate.tsv`: 100 skill rows (90 EQUAL) + 67 item/vendor rows (50 EQUAL), REC-2055/2056 rules;
  an EQUAL row needs all of its checks re-run; `last_verdict` matches ledger.py.
- `checks-status.md`: my fresh rows replace 65 hand-written and 61 gen-skill rows in place. `ledger.py --fix` then
  reconciled the stale verdicts this left in 11 other parts (last_verdict / state only).
## Open (gen-skill packets first differences, 23 checks)
- 9: 0xAC size at a summon's add (frame 27+; Valkyrie, Shadow, golems ...); 4: 0xA5 vs 0xA9 (pal-107 Charge ...).
- 3: 0x67 bytes[6] (frame 67); 2: 0xA8 missing; 2: 0xA3 missing. The missile wiring's `mark_state_changed` doesn't queue.
