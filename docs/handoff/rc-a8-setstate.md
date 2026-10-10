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

## Open (gen-skill packets first differences after the fix)
- 15: c2s 0x0C extra at frame 20 (rclick intent; not mine).
- 10 + 13 (S/M): paladin right aura at join. 1.14d `0x005701B0` -> `0x0056FF10` turns the aura state on with a list
  holding only markers 350 (skill) and 351 (level). The first update then sends it as 0xA8 after the join 0xAA.
  d2rs (`skill_events::right_aura_select`) either omits it (0xA8 "size 14 vs 17" at #72: Prayer, Holy Fire, Thorns ...)
  or puts it in the join 0xAA with the aura's stats ("0xAA size 12 vs 21-29" at #1: Might, Resist Fire ...).
  This ties in with REC-2105's join order. Not started.
- 8 + 5: 0xA7 missing / 0xA7 vs 0xAC at frame 27+ (summons and sentries); 6: 0xAC size; 6: 0x6B bytes[6]; 4: 0xA5 vs 0xA9.
