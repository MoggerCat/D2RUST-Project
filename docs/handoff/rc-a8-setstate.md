# rc-a8-setstate hand-back
Cause 1 (0xA8 byte 9): EQUAL 2759 -> 2801 on its base. Cause 2 (right aura at join): EQUAL 3195 -> 3213 on integ-r23.
gen-skill packets MATCH 91 -> 137 -> 178 of 210 (no regressions; gen-state-1? and gen-state-2? samples clean).

## Cause 1: S->C 0xA8 byte 9 at frame 2 (assassin masteries, ~60 checks)
- 1.14d `0x005711D0` (0xA8, sender `0x0053E8D0`) writes each entry as 9-bit stat id, param (send-param bits),
  value (send bits), ending with 0x1FF. d2rs' encoder already matched that; the list itself differed.
- 1.14d `0x00646D60` sets `passivestat1-5` on layer `passiveitype` (> 0, else 0): `0x00627150(list, s, v, layer)`.
  d2rs used layer 0, so it sent param 0 (byte 9: 1.14d 0x87 / 0x3D vs d2rs 0x01).
- Fix: `BodyWorld::list_set_layer` in `bodies::passive::refresh`. The mastery reader `mastery_of` (`0x00645830`)
  already filters on the layer. Spec `client/msg-skills.md` §2 r4 (+0xA8 note).
  Test: `tests6::passive_refresh_sets_the_stats_on_the_passiveitype_layer`.

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

## Ledger / status
- `ledger/rc-a8-setstate.tsv`: 100 skill rows, 83 EQUAL (REC-2055/2056: state 70/70, no ignore line, every
  channel incl. packets MATCH) and 17 DIVERGED. A DIVERGED row is settled by one re-run check; an EQUAL row
  needs all of its checks re-run. `last_verdict` is reconciled to ledger.py's value.
- `checks-status.md`: 44 rows refreshed in place. Held back because the fresh rows contradict other sessions'
  parts (net.s2c.0xa7 / 0xa9 / 0x7a and skill rows in q-run-net, q-tool-packet-census, rc-run-1, skills.tsv, ...):
  ass-burst-of-speed, ass-fade, ass-lightning-sentry, bar-leap-attack, pal-charge, pal-holy-freeze,
  pal-holy-shock, pal-sanctuary.

## Open (gen-skill packets first differences, 32 checks)
- 8 + 5: 0xA7 missing / 0xA7 vs 0xAC at frame 26+ (summons, sentries); 6: 0xAC size (frame 27+).
- 4: 0xA5 vs 0xA9 (incl. pal-107 Charge, frame 22); 3: 0x67 bytes[6] (frame 67); 2: 0xA8 missing; 2: 0xA3 missing.
- The missile wiring's `mark_state_changed` (`wiring/action/missiles.rs`) still doesn't queue (no game handle there).
