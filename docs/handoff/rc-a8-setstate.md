# rc-a8-setstate hand-back

EQUAL (merged ledger, after syncing with specs-staging) 2759 -> 2801 (before the sync: 2778 -> 2820). gen-skill packets MATCH 91 -> 137 of 210.

## Cause fixed: S->C 0xA8 byte 9 at frame 2 (assassin masteries, ~60 checks)
- 1.14d `0x005711D0` (0xA8 builder, sender `0x0053E8D0`) writes each state-list entry as
  9-bit stat id, then the param (ItemStatCost send-param bits), then the value (send bits), and ends with 0x1FF.
  d2rs' encoder already matched that.
- The difference was in the list itself. 1.14d `0x00646D60` (passive refresh) sets `passivestat1-5` with
  `0x00627150(list, s, v, layer)`, where layer = `passiveitype` if > 0, else 0. d2rs set them on layer 0
  (PROVISIONAL REC-150 in `passive.rs`), so 0xA8 sent param 0. Byte 9 = the stat id's high bit plus the
  first param bits: 1.14d 0x87 / 0x3D vs d2rs 0x01.
- Fix: `BodyWorld::list_set_layer`, used by `bodies::passive::refresh`. The weapon-mastery reader
  (`levels.rs` `mastery_of`, `0x00645830`) already filters on layer = item type.
  Spec: `specs/client/msg-skills.md` §2 rule 4 already said this; I added the 0xA8 cross-reference.
  Test: `tests6::passive_refresh_sets_the_stats_on_the_passiveitype_layer`.
- After the fix, every gen-skill-ass-* 0xA8 is byte-identical to 1.14d (e.g. gen-skill-ass-277 packets MATCH).

## Runs
- `suite.py --checks-dir traces/checks/gen --filter 'gen-skill-*' --orig-cache traces/orig-cache --fill-cache --workers 3`:
  packets MATCH 137, DIVERGED 73.
- The hand-written checks of the 0xA8 rows (43; `--filter <list>`): packets MATCH 32, DIVERGED 11.
- gen-state-1? sample (10 checks): 800/800 frames equal, packets MATCH 10. No regressions.
- d2-sim 4777 + 1 tests pass, d2-server 398 pass, clippy clean.

## Ledger / status
- `docs/handoff/ledger/rc-a8-setstate.tsv` has 106 skill rows: 56 EQUAL (REC-2055/2056: state 70/70 equal, no ignore line,
  every channel incl. packets MATCH) and 50 DIVERGED. A row is DIVERGED when one re-run check settles it; EQUAL
  only when all of the row's checks were re-run. `last_verdict` is reconciled to ledger.py's value from checks-status.md.
- `checks-status.md`: only the refreshed rows that don't create contradictions in other sessions' parts are kept
  (16 of 20). Held back: ass-burst-of-speed, ass-fade, ass-lightning-sentry, bar-leap-attack (now DIVERGED@46).
  Their fresh rows contradict net.s2c.0xa7/0xa9/0x7a and leap-attack rows in q-run-net, q-tool-packet-census, rc-run-1, skills.tsv.

## Cause 2 fixed: paladin right aura at join (C018 0xAA size, C026 0xA8 size; REC-3410)
- 1.14d (`intents-events.md` §8.2 r3.1, `d2s.md` §2.4 r6.3): the join's 0xAA goes out at player creation, and the
  post-load right-skill select `0x005701B0` -> `0x0056FF10` comes after it. Non-immediate: state on, list with only 350/351.
  Immediate: the do (`0x0056F7F0`) applies the stats, which the first 0x95 already shows (pal-115 Vigor stamina).
  The state reaches the client as 0xA8 at the first update.
- d2rs: the aura starts once at load (it ran twice). The non-immediate branch now makes the markers-only list
  (`UseView::set_aura_state`; the client seam only logged it before). The state's unit bit is hidden through the
  join and turned back on after the join messages, with the passive states (PROVISIONAL REC-3410, like REC-2105).
- `0x00639E30` queues the unit itself (`0x0064C040`), so the skill bodies' `mark_state_changed` now queues too:
  frame-51 0xA8 of Holy Fire / Freeze / Shock ... `sim/stat-lists.md` §9.2 corrected.
- gen-skill packets MATCH 137 -> 178/210 (pal 9 -> 29/30; no regressions). The hand-written 0xA8 rows' checks
  plus pal-* (69): packets MATCH 64. gen-state-2? sample clean. EQUAL on integ-r23: 3195 -> 3213 for this cause.

## Open (gen-skill packets first differences after the fix)
- pal-107 Charge: frame 22, 0xA5 vs 0xA9 (one check).
- 8 + 5: 0xA7 missing / 0xA7 vs 0xAC at frame 27+ (summons and sentries); 6: 0xAC size; 6: 0x6B bytes[6]; 4: 0xA5 vs 0xA9.
